"""Tests for loopflow.models — Wave and WaveRun parsing."""

from __future__ import annotations

from conftest import WAVE_FULL, WAVE_MINIMAL, WAVE_RUN_MINIMAL

from loopflow.models import Wave, WaveRun


class TestWaveModel:
    def test_minimal_payload(self):
        wave = Wave.model_validate(WAVE_MINIMAL)
        assert wave.name == "reduce"
        assert wave.stimulus is None
        assert wave.active_run is None
        assert wave.created_at is None

    def test_full_payload(self):
        wave = Wave.model_validate(WAVE_FULL)
        assert wave.stimulus.kind == "manual"
        assert wave.active_run.status == "running"
        assert wave.active_run.pr.number == 1
        assert wave.created_at is not None
        assert wave.branch == "wave/reduce"

    def test_round_trip(self):
        wave = Wave.model_validate(WAVE_FULL)
        dumped = wave.model_dump(mode="json")
        reparsed = Wave.model_validate(dumped)
        assert reparsed.id == wave.id
        assert reparsed.active_run.pr.url == wave.active_run.pr.url

    def test_unknown_fields_ignored(self):
        data = {**WAVE_MINIMAL, "new_field": "surprise"}
        wave = Wave.model_validate(data)
        assert wave.name == "reduce"


class TestWaveRunModel:
    def test_minimal_payload(self):
        run = WaveRun.model_validate(WAVE_RUN_MINIMAL)
        assert run.pr is None
        assert run.error is None
        assert run.flow_parents == []

    def test_with_error(self):
        data = {**WAVE_RUN_MINIMAL, "error": "rebase conflict"}
        run = WaveRun.model_validate(data)
        assert run.error == "rebase conflict"
