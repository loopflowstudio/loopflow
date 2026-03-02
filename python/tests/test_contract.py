"""Contract tests: golden JSON fixtures must parse through Python models."""

from __future__ import annotations

import json
from pathlib import Path

from loopflow.models import Trigger, Wave

FIXTURES = Path(__file__).resolve().parent.parent.parent / "tests" / "fixtures"


def _load(name: str) -> dict:
    return json.loads((FIXTURES / name).read_text())


def test_wave_fixture_parses():
    data = _load("wave.json")
    wave = Wave.model_validate(data)

    assert wave.id == "wave_abc123"
    assert wave.name == "engbot"
    assert wave.primary_flow == "build"
    assert wave.mode == "loop"
    assert wave.status == "running"
    assert wave.iteration == 3
    assert wave.direction == ["ux", "clarity"]
    assert wave.area == ["src/"]

    assert len(wave.triggers) == 2
    assert wave.triggers[0].signal == "repo"
    assert wave.triggers[0].flow == "integrate"
    assert wave.triggers[1].signal == "ci_failure"


def test_trigger_fixture_parses():
    data = _load("trigger.json")
    trigger = Trigger.model_validate(data)

    assert trigger.id == "trig_abc123"
    assert trigger.signal == "wave"
    assert trigger.flow == "build"
    assert trigger.source_wave_id == "wave_upstream"


def test_activation_log_fixture_has_expected_shape():
    data = _load("activation_log.json")

    assert data["object"] == "activation_log"
    assert data["wave_id"] == "wave_abc123"
    assert data["trigger_id"] == "trig_001"
    assert data["outcome"] == "started"
