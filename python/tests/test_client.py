"""Tests for loopflow.client — URL resolution, error handling, request formatting."""

from __future__ import annotations

import json

import httpx
import pytest

from conftest import WAVE_MINIMAL, WAVE_RUN_MINIMAL

from loopflow.client import Client, _extract_error_message, _resolve_base_url
from loopflow.errors import LoopflowError, WaveAlreadyRunning


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


def _mock_client(handler):
    transport = httpx.MockTransport(handler)
    c = Client.__new__(Client)
    c._base_url = "http://test"
    c._client = httpx.Client(base_url="http://test", transport=transport)
    return c


class TestClientErrors:
    def test_404_returns_none(self):
        def handler(request):
            return httpx.Response(404, json={"error": "not found"})

        client = _mock_client(handler)
        assert client.wave("missing") is None
        client.close()

    def test_412_raises_wave_already_running(self):
        def handler(request):
            return httpx.Response(412, json={"error": {"message": "wave is already running"}})

        client = _mock_client(handler)
        with pytest.raises(WaveAlreadyRunning, match="already running"):
            client.run_wave("test")
        client.close()

    def test_500_raises_loopflow_error(self):
        def handler(request):
            return httpx.Response(500, json={"error": "internal server error"})

        client = _mock_client(handler)
        with pytest.raises(LoopflowError, match="internal server error"):
            client.health()
        client.close()

    def test_connection_error(self):
        def handler(request):
            raise httpx.ConnectError("refused")

        client = _mock_client(handler)
        with pytest.raises(ConnectionError):
            client.health()
        client.close()


class TestClientResponses:
    def test_waves_parses_list(self):
        def handler(request):
            return httpx.Response(200, json={"object": "list", "data": [WAVE_MINIMAL]})

        client = _mock_client(handler)
        waves = client.waves()
        assert len(waves) == 1
        assert waves[0].name == "reduce"
        client.close()

    def test_wave_runs_parses_list(self):
        def handler(request):
            return httpx.Response(200, json={"object": "list", "data": [WAVE_RUN_MINIMAL]})

        client = _mock_client(handler)
        runs = client.wave_runs()
        assert len(runs) == 1
        assert runs[0].status == "completed"
        client.close()

    def test_create_wave_sends_correct_body(self):
        received = {}

        def handler(request):
            received.update(json.loads(request.content))
            return httpx.Response(200, json=WAVE_MINIMAL)

        client = _mock_client(handler)
        client.create_wave("reduce", "/tmp/repo", flow="reduce", direction=["ceo"])
        assert received["name"] == "reduce"
        assert received["direction"] == ["ceo"]
        assert "area" not in received
        client.close()


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
