"""Contract tests: golden JSON fixtures must parse through Python models."""

from __future__ import annotations

import json
from pathlib import Path

from loopflow.models import Wave

FIXTURES = Path(__file__).resolve().parent.parent.parent / "tests" / "fixtures"


def _load(name: str) -> dict:
    return json.loads((FIXTURES / name).read_text())


def test_wave_fixture_parses():
    data = _load("wave.json")
    wave = Wave.model_validate(data)

    assert wave.id == "wave_abc123"
    assert wave.name == "engbot"
    assert wave.goal == "ship-roadmap"
    assert wave.metrics == ["all roadmap items shipped", "cargo test green"]
    assert wave.status == "running"
    assert wave.direction == ["ux", "clarity"]
    assert wave.area == ["src/"]
    assert wave.parent_wave_id == "wave_parent999"

    assert wave.repo == "/home/user/project"
    assert wave.iteration == 3
    assert wave.active_run is None
