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
    def test_session_fixture_pins_process_registry_shape(self):
        session = Session.model_validate(_load("session.json"))
        assert session.object == "session"
        assert session.skill == "ship"
        assert session.agent == "codex"
        assert session.source == "lf_cli"
        assert session.session_use == "worker"
        assert session.status == "running"
        assert session.run_id is None
        assert session.parent_session_id is None
        assert session.argv == ["lf", "ship", "--wave", "Desktop"]
        assert session.env == {}

    def test_session_fixture_requires_argv_and_env(self):
        payload = _load("session.json")
        payload.pop("argv")
        payload.pop("env")

        with pytest.raises(ValidationError):
            Session.model_validate(payload)
