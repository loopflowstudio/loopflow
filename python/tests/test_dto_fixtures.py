"""Wire-shape fixture tests for DTOs mirrored across Rust / Python / Swift.

Each fixture under tests/fixtures/dto/ is parsed here and in the Rust and Swift
test suites. If any mirror drifts, one of the three fails.
"""

import json
from pathlib import Path

import pytest
from loopflow.models import Session
from pydantic import ValidationError

FIXTURE_DIR = Path(__file__).resolve().parents[2] / "tests" / "fixtures" / "dto"


def _load(name: str) -> dict:
    return json.loads((FIXTURE_DIR / name).read_text())


class TestDTOFixtures:
    def test_session_fixture_pins_palette_shape(self):
        session = Session.model_validate(_load("session.json"))
        assert session.object == "session"
        assert session.step == "ship"
        assert session.agent == "codex"
        assert session.source == "palette"
        assert session.session_use == "palette"
        assert session.status == "running"
        assert session.run_id is None
        assert session.parent_session_id is None
        assert session.argv == [
            "lf",
            "ship",
            "--no-direction",
            "-w",
            "Desktop",
            "-m",
            "codex",
        ]
        assert session.env["LFD_SESSION_ID"] == session.id

    def test_session_fixture_requires_argv_and_env(self):
        payload = _load("session.json")
        payload.pop("argv")
        payload.pop("env")

        with pytest.raises(ValidationError):
            Session.model_validate(payload)

    def test_create_session_request_fixture_has_required_keys(self):
        request = _load("create_session_request.json")
        assert request == {
            "wave_id": "lfdwave_01HNX7XYZ0AZ1B2C3D4E5F6G7H",
            "flow": "ship",
            "worktree": "/tmp/repo.Desktop",
            "agent": "codex",
        }
