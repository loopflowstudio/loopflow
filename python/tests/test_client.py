"""Tests for loopflow.client — URL resolution, error handling, request formatting."""

from __future__ import annotations

import json

import httpx
import pytest
from conftest import (
    AUTH_FLOW,
    AUTH_PROVIDER_ACTIVE,
    AUTH_PROVIDER_ACTIVE_WITH_TIMESTAMPS,
    AUTH_PROVIDER_NONE,
    CONVERSATION_MINIMAL,
    PROVIDER_INFO_FULL,
    PROVIDER_INFO_MINIMAL,
    REPO_MINIMAL,
    WAVE_MINIMAL,
    WAVE_RUN_MINIMAL,
)
from loopflow.client import Client, _extract_error_message, _resolve_base_url, _resolve_token
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


def _session_payload(**overrides):
    payload = {
        "id": "terminal-1",
        "object": "session",
        "wave_id": "abc-123",
        "run_id": None,
        "parent_session_id": None,
        "use": "wave_agent",
        "step": "goal",
        "agent": "lf",
        "cwd": "/tmp/repo",
        "argv": [],
        "env": {},
        "source": "wave_agent",
        "tmux_name": "lf-wave-agent",
        "status": "running",
        "created_at": "2026-07-02T00:00:00Z",
        "attached_at": None,
        "started_at": None,
        "completed_at": None,
    }
    payload.update(overrides)
    return payload


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

    def test_auth_status_parses_provider_timestamps(self):
        def handler(request):
            assert request.url.path == "/v0/auth/claude"
            return httpx.Response(200, json=AUTH_PROVIDER_ACTIVE_WITH_TIMESTAMPS)

        client = _mock_client(handler)
        status = client.auth_status("claude")
        assert status.expires_at is not None
        assert status.next_refresh_at is not None
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

    def test_complete_auth_posts_authorization_code(self):
        def handler(request):
            assert request.url.path == "/v0/auth/asana/complete"
            assert request.method == "POST"
            assert json.loads(request.content) == {"code": "auth-code-123"}
            return httpx.Response(200, json={"provider": "asana", "status": "accepted"})

        client = _mock_client(handler)
        client.complete_auth("asana", "auth-code-123")
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

    def test_providers_parses_list(self):
        def handler(request):
            assert request.url.path == "/v0/providers"
            return httpx.Response(
                200,
                json=[PROVIDER_INFO_MINIMAL, PROVIDER_INFO_FULL],
            )

        client = _mock_client(handler)
        providers = client.providers()
        assert len(providers) == 2
        assert providers[0].provider == "codex"
        assert providers[1].provider == "opencodezen"
        assert providers[1].models[0].id == "opencode/kimi-k2.5"
        client.close()

    def test_providers_invalid_payload_raises_error(self):
        def handler(request):
            return httpx.Response(200, json={"providers": [PROVIDER_INFO_MINIMAL]})

        client = _mock_client(handler)
        with pytest.raises(LoopflowError, match="invalid providers response payload"):
            client.providers()
        client.close()

    def test_waves_parses_list(self):
        def handler(request):
            return httpx.Response(200, json={"object": "list", "data": [WAVE_MINIMAL]})

        client = _mock_client(handler)
        waves = client.waves()
        assert len(waves) == 1
        assert waves[0].name == "reduce"
        client.close()

    def test_runs_parses_list(self):
        def handler(request):
            return httpx.Response(200, json={"object": "list", "data": [WAVE_RUN_MINIMAL]})

        client = _mock_client(handler)
        runs = client.runs()
        assert len(runs) == 1
        assert runs[0].status == "completed"
        client.close()

    def test_list_sessions_requests_active_sessions(self):
        def handler(request):
            assert request.url.path == "/v0/sessions"
            assert request.url.params.get("wave_id") == "abc-123"
            assert request.url.params.get("parent_session_id") == "terminal-parent"
            assert request.url.params.get("use") == "worker"
            assert request.url.params.get("active_only") == "true"
            return httpx.Response(
                200,
                json={
                    "object": "list",
                    "data": [
                        _session_payload(id="terminal-1", status="running"),
                    ],
                },
            )

        client = _mock_client(handler)
        sessions = client.list_sessions(
            wave_id="abc-123",
            parent_session_id="terminal-parent",
            use="worker",
        )
        assert [session.id for session in sessions] == ["terminal-1"]
        client.close()

    def test_get_wave_agent_tree_parses_sessions_and_connection(self):
        def handler(request):
            assert request.url.path == "/v0/waves/reduce/agent-tree"
            assert request.url.params.get("active_only") == "true"
            return httpx.Response(
                200,
                json={
                    "object": "wave_agent_tree",
                    "id": "tree-abc-123",
                    "wave": WAVE_MINIMAL,
                    "child_waves": [],
                    "sessions": [
                        {
                            "session": _session_payload(
                                id="terminal-worker",
                                use="worker",
                                parent_session_id="terminal-1",
                            ),
                            "connection": {
                                "kind": "tmux",
                                "session_name": "lf-worker",
                                "host": "localhost",
                                "cwd": "/tmp/repo",
                                "status": "running",
                            },
                        }
                    ],
                },
            )

        client = _mock_client(handler)
        tree = client.get_wave_agent_tree("reduce")
        assert tree.object == "wave_agent_tree"
        assert tree.wave.name == "reduce"
        assert tree.sessions[0].session.parent_session_id == "terminal-1"
        assert tree.sessions[0].connection is not None
        assert tree.sessions[0].connection.session_name == "lf-worker"
        client.close()

    def test_run_worker_posts_launch_request(self):
        received = {}

        def handler(request):
            assert request.url.path == "/v0/waves/reduce/workers"
            assert request.method == "POST"
            received.update(json.loads(request.content))
            return httpx.Response(
                200,
                json=_session_payload(
                    run_id="run-1",
                    use="worker",
                    source="wave_step_tmux",
                    step="implement",
                    tmux_name="lf-worker",
                ),
            )

        client = _mock_client(handler)
        response = client.run_worker(
            "reduce",
            "implement",
            "Add the endpoint",
            parent_session_id="terminal-parent",
        )
        assert received == {
            "flow": "implement",
            "task": "Add the endpoint",
            "parent_session_id": "terminal-parent",
        }
        assert response.id == "terminal-1"
        assert response.session_use == "worker"
        client.close()

    def test_get_session_returns_dict(self):
        def handler(request):
            assert request.url.path == "/v0/sessions/terminal-1"
            return httpx.Response(
                200,
                json=_session_payload(use="worker"),
            )

        client = _mock_client(handler)
        session = client.get_session("terminal-1")
        assert session.session_use == "worker"
        client.close()

    def test_current_session_uses_cwd_lookup(self):
        def handler(request):
            assert request.url.path == "/v0/sessions/current"
            assert request.url.params.get("cwd") == "/tmp/repo/child"
            return httpx.Response(
                200,
                json=_session_payload(),
            )

        client = _mock_client(handler)
        session = client.current_session("/tmp/repo/child")
        assert session is not None
        assert session.id == "terminal-1"
        client.close()

    def test_current_session_returns_none_for_not_found(self):
        def handler(request):
            assert request.url.path == "/v0/sessions/current"
            return httpx.Response(404, json={"error": {"message": "not found"}})

        client = _mock_client(handler)
        assert client.current_session("/tmp/repo") is None
        client.close()

    def test_attach_session_returns_connection_info(self):
        def handler(request):
            assert request.url.path == "/v0/sessions/terminal-1/attach"
            assert request.method == "POST"
            return httpx.Response(
                200,
                json={
                    "kind": "tmux",
                    "session_name": "lfq-terminal-1",
                    "host": "localhost",
                    "cwd": "/tmp/repo",
                    "status": "attached",
                },
            )

        client = _mock_client(handler)
        connection = client.attach_session("terminal-1")
        assert connection.session_name == "lfq-terminal-1"
        client.close()

    def test_list_attention_returns_dicts(self):
        def handler(request):
            assert request.url.path == "/v0/attention"
            assert request.url.params.get("status") == "unresolved"
            return httpx.Response(
                200,
                json={
                    "object": "list",
                    "data": [
                        {
                            "id": "attention-1",
                            "kind": "interactive",
                            "status": "surfaced",
                            "context": {"session_id": "terminal-1"},
                        }
                    ],
                },
            )

        client = _mock_client(handler)
        items = client.list_attention(status="unresolved")
        assert items[0]["context"]["session_id"] == "terminal-1"
        client.close()

    def test_waves_invalid_list_payload_raises_error(self):
        def handler(request):
            return httpx.Response(200, json=[])

        client = _mock_client(handler)
        with pytest.raises(LoopflowError, match="invalid list response payload"):
            client.waves()
        client.close()

    def test_create_wave_sends_correct_body(self):
        received = {}

        def handler(request):
            received.update(json.loads(request.content))
            return httpx.Response(200, json=WAVE_MINIMAL)

        client = _mock_client(handler)
        client.create_wave(
            "reduce",
            "/tmp/repo",
            flow="reduce",
            direction=["ceo"],
            status="paused",
        )
        assert received["name"] == "reduce"
        assert received["direction"] == ["ceo"]
        assert received["status"] == "paused"
        assert "area" not in received
        client.close()

    def test_create_wave_includes_crons_when_provided(self):
        received = {}

        def handler(request):
            received.update(json.loads(request.content))
            return httpx.Response(200, json=WAVE_MINIMAL)

        client = _mock_client(handler)
        client.create_wave(
            "reduce",
            "/tmp/repo",
            flow="reduce",
            crons=[{"flow": "wave-polish", "schedule": "0 0 * * 1"}],
        )
        assert received["crons"] == [{"flow": "wave-polish", "schedule": "0 0 * * 1"}]
        client.close()

    def test_update_wave_includes_crons_when_provided(self):
        received = {}

        def handler(request):
            received.update(json.loads(request.content))
            return httpx.Response(200, json=WAVE_MINIMAL)

        client = _mock_client(handler)
        client.update_wave(
            "reduce",
            flow="reduce",
            crons=[{"flow": "wave-reduce", "schedule": "0 0 1 * *"}],
        )
        assert received["crons"] == [{"flow": "wave-reduce", "schedule": "0 0 1 * *"}]
        client.close()

    def test_add_trigger_with_wave_source_sends_correct_body(self):
        received = {}

        def handler(request):
            received.update(json.loads(request.content))
            return httpx.Response(200, json={"id": "trig-1", "signal": "wave"})

        client = _mock_client(handler)
        client.add_trigger("designer", "wave", source_wave_id="infra")
        assert received["signal"] == "wave"
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

    def test_repos_mutations_and_list(self):
        requests = []

        def handler(request):
            requests.append((request.method, request.url.path, request.content))
            if request.method == "GET":
                return httpx.Response(200, json={"object": "list", "data": [REPO_MINIMAL]})
            if request.method == "POST":
                return httpx.Response(201, json=REPO_MINIMAL)
            return httpx.Response(204)

        client = _mock_client(handler)
        repos = client.list_repos()
        created = client.add_repo("/tmp/repo")
        client.remove_repo("/tmp/repo")

        assert len(repos) == 1
        assert repos[0].registered is True
        assert created.path == "/tmp/repo"
        assert requests[0][0:2] == ("GET", "/v0/repos")
        assert requests[1][0:2] == ("POST", "/v0/repos")
        assert json.loads(requests[1][2])["path"] == "/tmp/repo"
        assert requests[2][0:2] == ("DELETE", "/v0/repos")
        client.close()

    def test_repo_child_routes(self):
        requests = []

        def handler(request):
            requests.append((request.method, request.url.path))
            if request.method == "GET":
                return httpx.Response(200, json={"object": "list", "data": [REPO_MINIMAL]})
            return httpx.Response(204)

        client = _mock_client(handler)
        client.add_child("loopflowstudio", "studio", "loopflowstudio", "loopflow")
        children = client.list_children("loopflowstudio", "studio")
        parents = client.list_parents("loopflowstudio", "loopflow")
        client.remove_child("loopflowstudio", "studio", "loopflowstudio", "loopflow")

        assert len(children) == 1
        assert len(parents) == 1
        assert requests == [
            ("POST", "/v0/repos/loopflowstudio/studio/children/loopflowstudio/loopflow"),
            ("GET", "/v0/repos/loopflowstudio/studio/children"),
            ("GET", "/v0/repos/loopflowstudio/loopflow/parents"),
            ("DELETE", "/v0/repos/loopflowstudio/studio/children/loopflowstudio/loopflow"),
        ]
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
        base_url=base_url,
        transport=transport,
        headers=client._client.headers,
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
