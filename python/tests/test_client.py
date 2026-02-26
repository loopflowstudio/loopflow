"""Tests for loopflow.client — URL resolution, error handling, request formatting."""

from __future__ import annotations

import json

import httpx
import pytest

from conftest import (
    AUTH_FLOW,
    AUTH_PROVIDER_ACTIVE,
    AUTH_PROVIDER_NONE,
    CHORD_MINIMAL,
    SESSION_MINIMAL,
    WAVE_MINIMAL,
    WAVE_RUN_MINIMAL,
)

from loopflow.client import Client, _extract_error_message, _resolve_base_url, _resolve_token
from loopflow.errors import LoopflowError, WaveAlreadyRunning
from loopflow.models import SessionConfig


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


class TestTokenResolution:
    def test_env_token_wins(self, monkeypatch, tmp_path):
        monkeypatch.setenv("LFD_TOKEN", "env-token")
        monkeypatch.setattr("loopflow.client.Path.home", lambda: tmp_path)

        token_file = tmp_path / ".lf" / "session-token"
        token_file.parent.mkdir(parents=True)
        token_file.write_text("file-token")

        assert _resolve_token("http://127.0.0.1:2486") == "env-token"

    def test_reads_session_token_file(self, monkeypatch, tmp_path):
        monkeypatch.delenv("LFD_TOKEN", raising=False)
        monkeypatch.setattr("loopflow.client.Path.home", lambda: tmp_path)

        token_file = tmp_path / ".lf" / "session-token"
        token_file.parent.mkdir(parents=True)
        token_file.write_text(" file-token\n")

        assert _resolve_token("http://127.0.0.1:2486") == "file-token"

    def test_missing_session_token_file_returns_none(self, monkeypatch, tmp_path):
        monkeypatch.delenv("LFD_TOKEN", raising=False)
        monkeypatch.setattr("loopflow.client.Path.home", lambda: tmp_path)

        assert _resolve_token("http://127.0.0.1:2486") is None

    def test_remote_base_url_does_not_read_session_token(self, monkeypatch, tmp_path):
        monkeypatch.delenv("LFD_TOKEN", raising=False)
        monkeypatch.setattr("loopflow.client.Path.home", lambda: tmp_path)

        token_file = tmp_path / ".lf" / "session-token"
        token_file.parent.mkdir(parents=True)
        token_file.write_text("file-token")

        assert _resolve_token("https://lfd.example.com") is None


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
    def test_auth_status_parses_all_providers(self):
        def handler(request):
            assert request.url.path == "/v0/auth"
            return httpx.Response(
                200,
                json={"providers": [AUTH_PROVIDER_ACTIVE, AUTH_PROVIDER_NONE]},
            )

        client = _mock_client(handler)
        statuses = client.auth_status()
        assert len(statuses) == 2
        assert statuses[0].provider == "github"
        assert statuses[0].status == "active"
        assert statuses[0].login == "jackdanger"
        client.close()

    def test_auth_status_parses_single_provider(self):
        def handler(request):
            assert request.url.path == "/v0/auth/github"
            return httpx.Response(200, json=AUTH_PROVIDER_ACTIVE)

        client = _mock_client(handler)
        status = client.auth_status("github")
        assert status.provider == "github"
        assert status.status == "active"
        client.close()

    def test_start_auth_parses_flow_response(self):
        def handler(request):
            assert request.url.path == "/v0/auth/github"
            assert request.method == "POST"
            return httpx.Response(200, json=AUTH_FLOW)

        client = _mock_client(handler)
        flow = client.start_auth("github")
        assert flow.provider == "github"
        assert flow.user_code == "ABCD-1234"
        client.close()

    def test_disconnect_auth_returns_status(self):
        def handler(request):
            assert request.url.path == "/v0/auth/github"
            assert request.method == "DELETE"
            return httpx.Response(200, json={"provider": "github", "status": "none"})

        client = _mock_client(handler)
        status = client.disconnect_auth("github")
        assert status.provider == "github"
        assert status.status == "none"
        client.close()

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

    def test_add_stimulus_with_listen_source_sends_correct_body(self):
        received = {}

        def handler(request):
            received.update(json.loads(request.content))
            return httpx.Response(200, json={"id": "stim-1", "kind": "listen"})

        client = _mock_client(handler)
        client.add_stimulus("designer", "listen", source_wave_id="infra")
        assert received["kind"] == "listen"
        assert received["source_wave_id"] == "infra"
        client.close()

    def test_next_wave(self):
        def handler(request):
            assert request.url.path == "/v0/waves/reduce/next"
            assert request.method == "POST"
            return httpx.Response(200, json={"new_branch": "wave/reduce.2"})

        client = _mock_client(handler)
        result = client.next_wave("reduce")
        assert result["new_branch"] == "wave/reduce.2"
        client.close()

    def test_create_chord_sends_correct_body(self):
        received = {}

        def handler(request):
            received.update(json.loads(request.content))
            return httpx.Response(200, json=CHORD_MINIMAL)

        client = _mock_client(handler)
        chord = client.create_chord("ensemble-a")
        assert received["name"] == "ensemble-a"
        assert chord.name == "ensemble-a"
        client.close()

    def test_list_chords_parses_list(self):
        def handler(request):
            return httpx.Response(200, json={"object": "list", "data": [CHORD_MINIMAL]})

        client = _mock_client(handler)
        chords = client.list_chords()
        assert len(chords) == 1
        assert chords[0].name == "ensemble-a"
        client.close()

    def test_get_chord_404_returns_none(self):
        def handler(request):
            return httpx.Response(404, json={"error": "not found"})

        client = _mock_client(handler)
        assert client.get_chord("missing") is None
        client.close()

    def test_chord_membership_mutations_handle_204(self):
        requests = []

        def handler(request):
            requests.append((request.method, request.url.path))
            return httpx.Response(204)

        client = _mock_client(handler)
        client.add_chord_member("chord-1", "wave-1")
        client.remove_chord_member("chord-1", "wave-1")
        client.delete_chord("chord-1")
        assert requests == [
            ("POST", "/v0/chords/chord-1/members"),
            ("DELETE", "/v0/chords/chord-1/members/wave-1"),
            ("DELETE", "/v0/chords/chord-1"),
        ]
        client.close()

    def test_create_session_sends_correct_body(self):
        received = {}

        def handler(request):
            received.update(json.loads(request.content))
            return httpx.Response(200, json=SESSION_MINIMAL)

        client = _mock_client(handler)
        config = SessionConfig(model="claude-sonnet", yolo_mode=True)
        session = client.create_session("claude", wave_run_id="run-1", config=config)

        assert received["harness"] == "claude"
        assert received["wave_run_id"] == "run-1"
        assert received["config"]["model"] == "claude-sonnet"
        assert received["config"]["yolo_mode"] is True
        assert session.id == "session-1"
        client.close()

    def test_session_404_returns_none(self):
        def handler(request):
            return httpx.Response(404, json={"error": "not found"})

        client = _mock_client(handler)
        assert client.session("missing") is None
        client.close()

    def test_send_session_input_sends_correct_body(self):
        received = {}

        def handler(request):
            received.update(json.loads(request.content))
            return httpx.Response(200, json=SESSION_MINIMAL)

        client = _mock_client(handler)
        client.send_session_input("session-1", "hello")
        assert received["content"] == "hello"
        client.close()

    def test_stream_session_events_parses_sse(self):
        def handler(request):
            return httpx.Response(
                200,
                text='\n'.join([
                    "id: 0",
                    'data: {"type":"status_changed","status":"starting"}',
                    "",
                    "id: 1",
                    'data: {"type":"turn_completed","status":"completed"}',
                    "",
                ]),
            )

        client = _mock_client(handler)
        events = list(client.stream_session_events("session-1", timeout=1))

        assert len(events) == 2
        assert events[0].seq == 0
        assert events[0].event["type"] == "status_changed"
        assert events[1].seq == 1
        assert events[1].event["type"] == "turn_completed"
        client.close()

    def test_stream_session_events_sends_after_seq(self):
        def handler(request):
            assert request.url.params.get("after_seq") == "10"
            return httpx.Response(200, text='data: {"type":"turn_completed"}\n')

        client = _mock_client(handler)
        events = list(client.stream_session_events("session-1", after_seq=10, timeout=1))
        assert len(events) == 1
        client.close()

    def test_stream_session_events_errors(self):
        def handler(request):
            return httpx.Response(500, json={"error": "boom"})

        client = _mock_client(handler)
        with pytest.raises(LoopflowError, match="boom"):
            list(client.stream_session_events("session-1", timeout=1))
        client.close()


def _mock_token_client(token=None, base_url="http://test"):
    """Create a Client with a mock transport that captures request headers."""
    received_headers = {}

    def handler(request):
        received_headers.update(dict(request.headers))
        return httpx.Response(200, json={"status": "ok"})

    transport = httpx.MockTransport(handler)
    client = Client(base_url=base_url, token=token)
    client._client = httpx.Client(
        base_url=base_url, transport=transport, headers=client._client.headers,
    )
    return client, received_headers


class TestClientToken:
    def test_token_kwarg_sends_bearer_header(self):
        client, headers = _mock_token_client(token="my-secret-token")
        client.health()
        assert headers.get("authorization") == "Bearer my-secret-token"
        client.close()

    def test_lfd_token_env_sends_bearer_header(self, monkeypatch):
        monkeypatch.setenv("LFD_TOKEN", "env-token-123")
        client, headers = _mock_token_client()
        client.health()
        assert headers.get("authorization") == "Bearer env-token-123"
        client.close()

    def test_token_kwarg_overrides_env(self, monkeypatch):
        monkeypatch.setenv("LFD_TOKEN", "env-token")
        client, headers = _mock_token_client(token="kwarg-token")
        client.health()
        assert headers.get("authorization") == "Bearer kwarg-token"
        client.close()

    def test_no_token_sends_no_auth_header(self, monkeypatch):
        monkeypatch.delenv("LFD_TOKEN", raising=False)
        monkeypatch.setattr("loopflow.client._resolve_token", lambda _base_url: None)
        client, headers = _mock_token_client()
        client.health()
        assert "authorization" not in headers
        client.close()

    def test_session_token_file_sends_bearer_on_local_url(self, monkeypatch, tmp_path):
        monkeypatch.delenv("LFD_TOKEN", raising=False)
        monkeypatch.setattr("loopflow.client.Path.home", lambda: tmp_path)

        token_file = tmp_path / ".lf" / "session-token"
        token_file.parent.mkdir(parents=True)
        token_file.write_text("file-session-token")

        client, headers = _mock_token_client(base_url="http://127.0.0.1:2486")
        client.health()
        assert headers.get("authorization") == "Bearer file-session-token"
        client.close()

    def test_session_token_file_not_sent_to_remote_url(self, monkeypatch, tmp_path):
        monkeypatch.delenv("LFD_TOKEN", raising=False)
        monkeypatch.setattr("loopflow.client.Path.home", lambda: tmp_path)

        token_file = tmp_path / ".lf" / "session-token"
        token_file.parent.mkdir(parents=True)
        token_file.write_text("file-session-token")

        client, headers = _mock_token_client(base_url="https://lfd.example.com")
        client.health()
        assert "authorization" not in headers
        client.close()

    def test_client_does_not_follow_redirects(self):
        client = Client(base_url="https://lfd.example.com", token="redirect-safe-token")
        assert client._client.follow_redirects is False
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
