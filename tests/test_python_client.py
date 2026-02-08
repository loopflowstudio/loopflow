"""Tests for the loopflow Python client: model validation, URL resolution, error handling."""

from __future__ import annotations

import json

import httpx
import pytest

from loopflow.client import Client, _extract_error_message, _resolve_base_url
from loopflow.errors import LoopflowError, WaveAlreadyRunning
from loopflow.models import Wave, WaveRun


# ── Fixtures: realistic API payloads ──────────────────────────────────────


WAVE_MINIMAL = {
    "id": "abc-123",
    "name": "reduce",
    "repo": "/tmp/repo",
    "flow": "reduce",
    "direction": ["infra-engineer"],
    "area": [],
    "status": "running",
    "iteration": 0,
}

WAVE_FULL = {
    **WAVE_MINIMAL,
    "stimulus": {"kind": "manual"},
    "created_at": "2026-02-08T01:56:54Z",
    "active_run": {
        "id": "run-1",
        "wave_id": "abc-123",
        "iteration": 0,
        "step_index": 0,
        "status": "running",
        "local_worktree": "/tmp/wt",
        "remote_branch": "wave/reduce",
        "pr": {"url": "https://github.com/org/repo/pull/1", "number": 1, "state": "open"},
        "started_at": "2026-02-08T02:00:00Z",
        "flow_parents": ["parent-1"],
    },
    "branch": "wave/reduce",
    "pr_url": "https://github.com/org/repo/pull/1",
    "pr_state": "open",
}

WAVE_RUN_MINIMAL = {
    "id": "run-1",
    "wave_id": "abc-123",
    "iteration": 0,
    "step_index": 0,
    "status": "completed",
    "local_worktree": "/tmp/wt",
    "remote_branch": "wave/reduce",
}


# ── Model validation ─────────────────────────────────────────────────────


class TestWaveModel:
    def test_minimal_payload(self):
        """Wave parses without stimulus, active_run, or timestamps."""
        wave = Wave.model_validate(WAVE_MINIMAL)
        assert wave.name == "reduce"
        assert wave.stimulus is None
        assert wave.active_run is None
        assert wave.created_at is None

    def test_full_payload(self):
        """Wave parses with all fields populated, including nested models."""
        wave = Wave.model_validate(WAVE_FULL)
        assert wave.stimulus.kind == "manual"
        assert wave.active_run.status == "running"
        assert wave.active_run.pr.number == 1
        assert wave.created_at is not None
        assert wave.branch == "wave/reduce"

    def test_round_trip(self):
        """model_dump(mode='json') produces valid JSON that re-parses."""
        wave = Wave.model_validate(WAVE_FULL)
        dumped = wave.model_dump(mode="json")
        reparsed = Wave.model_validate(dumped)
        assert reparsed.id == wave.id
        assert reparsed.active_run.pr.url == wave.active_run.pr.url

    def test_unknown_fields_ignored(self):
        """Extra fields from a newer API version don't break parsing."""
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


# ── URL resolution ────────────────────────────────────────────────────────


class TestUrlResolution:
    def test_defaults(self, monkeypatch):
        monkeypatch.delenv("LFD_URL", raising=False)
        monkeypatch.delenv("LFD_HOST", raising=False)
        monkeypatch.delenv("LFD_PORT", raising=False)
        assert _resolve_base_url() == "http://127.0.0.1:2486"

    def test_lfd_url(self, monkeypatch):
        monkeypatch.setenv("LFD_URL", "http://custom:9999/")
        assert _resolve_base_url() == "http://custom:9999"

    def test_host_and_port(self, monkeypatch):
        monkeypatch.delenv("LFD_URL", raising=False)
        monkeypatch.setenv("LFD_HOST", "10.0.0.1")
        monkeypatch.setenv("LFD_PORT", "3000")
        assert _resolve_base_url() == "http://10.0.0.1:3000"

    def test_client_strips_trailing_slash(self):
        client = Client(base_url="http://localhost:2486/")
        assert client._base_url == "http://localhost:2486"
        client.close()


# ── Client error handling ─────────────────────────────────────────────────


def _mock_transport(handler):
    """Create an httpx.MockTransport from a handler function."""
    return httpx.MockTransport(handler)


class TestClientErrors:
    def _client_with(self, handler):
        transport = _mock_transport(handler)
        c = Client.__new__(Client)
        c._base_url = "http://test"
        c._client = httpx.Client(base_url="http://test", transport=transport)
        return c

    def test_404_returns_none(self):
        def handler(request):
            return httpx.Response(404, json={"error": "not found"})

        client = self._client_with(handler)
        assert client.wave("missing") is None
        client.close()

    def test_412_raises_wave_already_running(self):
        def handler(request):
            return httpx.Response(412, json={"error": {"message": "wave is already running"}})

        client = self._client_with(handler)
        with pytest.raises(WaveAlreadyRunning, match="already running"):
            client.run_wave("test")
        client.close()

    def test_500_raises_loopflow_error(self):
        def handler(request):
            return httpx.Response(500, json={"error": "internal server error"})

        client = self._client_with(handler)
        with pytest.raises(LoopflowError, match="internal server error"):
            client.health()
        client.close()

    def test_connection_error(self):
        def handler(request):
            raise httpx.ConnectError("refused")

        client = self._client_with(handler)
        with pytest.raises(ConnectionError):
            client.health()
        client.close()

    def test_waves_parses_list_response(self):
        def handler(request):
            return httpx.Response(200, json={"object": "list", "data": [WAVE_MINIMAL]})

        client = self._client_with(handler)
        waves = client.waves()
        assert len(waves) == 1
        assert waves[0].name == "reduce"
        client.close()

    def test_wave_runs_parses_list_response(self):
        def handler(request):
            return httpx.Response(200, json={"object": "list", "data": [WAVE_RUN_MINIMAL]})

        client = self._client_with(handler)
        runs = client.wave_runs()
        assert len(runs) == 1
        assert runs[0].status == "completed"
        client.close()

    def test_create_wave_sends_correct_body(self):
        received = {}

        def handler(request):
            received.update(json.loads(request.content))
            return httpx.Response(200, json=WAVE_MINIMAL)

        client = self._client_with(handler)
        client.create_wave("reduce", "/tmp/repo", flow="reduce", direction=["ceo"])
        assert received["name"] == "reduce"
        assert received["direction"] == ["ceo"]
        assert "area" not in received  # None args omitted
        client.close()


# ── Error message extraction ──────────────────────────────────────────────


class TestExtractErrorMessage:
    def test_nested_error_message(self):
        resp = httpx.Response(400, json={"error": {"message": "bad request"}})
        assert _extract_error_message(resp) == "bad request"

    def test_string_error(self):
        resp = httpx.Response(400, json={"error": "something went wrong"})
        assert _extract_error_message(resp) == "something went wrong"

    def test_non_json_response(self):
        resp = httpx.Response(500, text="gateway timeout")
        assert _extract_error_message(resp) == "gateway timeout"

    def test_empty_response(self):
        resp = httpx.Response(500, text="")
        assert _extract_error_message(resp) == "HTTP 500"
