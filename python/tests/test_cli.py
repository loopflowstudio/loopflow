"""Tests for loopflow.cli table rendering."""

from __future__ import annotations

import json

import pytest
import typer
from conftest import (
    AUTH_PROVIDER_ACTIVE,
    AUTH_PROVIDER_ACTIVE_WITH_TIMESTAMPS,
    AUTH_PROVIDER_NONE,
    PROVIDER_INFO_FULL,
    PROVIDER_INFO_MINIMAL,
    REPO_MINIMAL,
    WAVE_FULL,
    WAVE_MINIMAL,
)
from loopflow.cli import (
    _auth_poll_timeout_seconds,
    _auth_status_table,
    _extract_authorization_code,
    _pm_oauth_configure_error,
    _provider_api_key_config,
    _providers_table,
    _repo_table,
    _sessions_table,
    _split_repo_slug,
    _status_details,
    _wave_detail_table,
    _wave_table,
    app,
)
from loopflow.errors import LoopflowError
from loopflow.models import (
    AuthProviderStatus,
    ProviderInfo,
    Repo,
    Session,
    SessionConnectionInfo,
    Wave,
)
from rich.console import Console
from typer.testing import CliRunner


def _render_table(wave: Wave) -> str:
    console = Console(record=True, width=220)
    console.print(_wave_table([wave]))
    return console.export_text()


def _session_payload(**overrides: object) -> dict[str, object]:
    payload: dict[str, object] = {
        "id": "terminal-session-agent",
        "object": "session",
        "wave_id": "abc-123",
        "run_id": None,
        "parent_session_id": None,
        "use": "wave_agent",
        "source": "wave_agent",
        "step": "goal",
        "agent": "lf",
        "cwd": "/tmp/repo",
        "argv": [],
        "env": {},
        "tmux_name": "lf-wave-agent",
        "status": "running",
        "created_at": "2026-07-02T00:00:00Z",
        "attached_at": None,
        "started_at": None,
        "completed_at": None,
    }
    payload.update(overrides)
    return payload


def _session(**overrides: object) -> Session:
    return Session.model_validate(_session_payload(**overrides))


def _worker_launch_response(flow: str = "implement") -> Session:
    return Session.model_validate(
        _session_payload(
            id="terminal-session-child",
            run_id="run-1",
            parent_session_id="terminal-session-agent",
            use="worker",
            source="wave_dispatch",
            step=flow,
            tmux_name="lf-worker",
        )
    )


def test_wave_table_includes_worktree_and_branch_columns() -> None:
    wave = Wave.model_validate(WAVE_MINIMAL)
    rendered = _render_table(wave)

    assert "local_worktree" in rendered
    assert "remote_branch" in rendered


def test_wave_table_uses_active_run_paths_when_available() -> None:
    wave = Wave.model_validate(WAVE_FULL)
    rendered = _render_table(wave)

    assert "/tmp/wt" in rendered
    assert "wave/reduce" in rendered


def test_wave_table_falls_back_to_repo_branch() -> None:
    payload = dict(WAVE_MINIMAL)
    repo = {**WAVE_MINIMAL["repos"][0], "remote_branch": "wave/fallback", "local_worktree": "/tmp/fallback-wt"}
    payload["repos"] = [repo]
    wave = Wave.model_validate(payload)
    rendered = _render_table(wave)

    assert "wave/fallback" in rendered
    assert "/tmp/fallback-wt" in rendered


def test_wave_detail_table_includes_flow_and_area() -> None:
    wave = Wave.model_validate(
        {
            **WAVE_MINIMAL,
            "area": ["wave/chord-model/", "wave/signals/"],
            "direction": ["care", "clarity"],
            "primary_flow": "tend",
        }
    )
    console = Console(record=True, width=220)
    console.print(_wave_detail_table(wave))
    rendered = console.export_text()

    assert "flow" in rendered
    assert "tend" in rendered
    assert "area" in rendered
    assert "wave/chord-model/, wave/signals/" in rendered


def test_sessions_table_flags_interactive_attention() -> None:
    sessions = [
        _session(),
        _session(
            id="terminal-session-child",
            run_id="run-1",
            use="worker",
            source="wave_dispatch",
            step="implement",
            status="attached",
        ),
    ]
    attention_items = [
        {
            "kind": "interactive",
            "status": "surfaced",
            "context": {"session_id": "terminal-session-child"},
        }
    ]
    console = Console(record=True, width=220)
    console.print(_sessions_table(sessions, attention_items, {"abc-123": "reduce"}))
    rendered = console.export_text()

    assert "reduce" in rendered
    assert "wave_agent" in rendered
    assert "worker" in rendered
    assert "implement" in rendered
    assert "yes" in rendered


def test_sessions_command_renders_live_sessions(monkeypatch: pytest.MonkeyPatch) -> None:
    wave = Wave.model_validate(WAVE_MINIMAL)
    sessions = [
        _session(
            id="terminal-session-child",
            wave_id=wave.id,
            run_id="run-1",
            use="worker",
            source="wave_dispatch",
            step="implement",
        )
    ]
    attention_items = [
        {
            "kind": "interactive",
            "status": "surfaced",
            "context": {"session_id": "terminal-session-child"},
        }
    ]
    monkeypatch.setattr("loopflow.cli.api.wave", lambda _name: wave)
    monkeypatch.setattr("loopflow.cli.api.list_sessions", lambda **_kwargs: sessions)
    monkeypatch.setattr("loopflow.cli.api.list_attention", lambda **_kwargs: attention_items)

    result = CliRunner().invoke(app, ["sessions", "reduce"])

    assert result.exit_code == 0
    assert "reduce" in result.stdout
    assert "implement" in result.stdout
    assert "yes" in result.stdout


def test_whoami_renders_current_session(monkeypatch: pytest.MonkeyPatch) -> None:
    session = _session()
    monkeypatch.setenv("LFD_SESSION_ID", "terminal-session-agent")
    monkeypatch.setattr("loopflow.cli.api.get_session", lambda _session_id: session)

    result = CliRunner().invoke(app, ["whoami"])

    assert result.exit_code == 0
    assert "terminal-session-agent" in result.stdout
    assert "wave_agent" in result.stdout


def test_worker_run_dispatches_and_prints_session(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        "loopflow.cli.api.run_worker",
        lambda wave, flow, task, parent_session_id=None: _worker_launch_response(flow),
    )

    result = CliRunner().invoke(
        app,
        ["worker", "run", "reduce", "--flow", "implement", "--task", "Add the endpoint"],
    )

    assert result.exit_code == 0
    assert "worker_run terminal-session-child" in result.stdout
    assert "run     run-1" in result.stdout
    assert "lfq attach terminal-session-child" in result.stdout


def test_worker_run_json_includes_session_connection(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        "loopflow.cli.api.run_worker",
        lambda _wave, flow, task, parent_session_id=None: _worker_launch_response(flow),
    )

    result = CliRunner().invoke(
        app,
        [
            "worker",
            "run",
            "reduce",
            "--flow",
            "implement",
            "--task",
            "Add the endpoint",
            "--json",
        ],
    )

    assert result.exit_code == 0
    payload = json.loads(result.stdout)
    assert payload["id"] == "terminal-session-child"
    assert payload["run_id"] == "run-1"
    assert payload["tmux_name"] == "lf-worker"


def test_worker_run_infers_wave_from_current_session(monkeypatch: pytest.MonkeyPatch) -> None:
    received: dict[str, object] = {}

    monkeypatch.setattr(
        "loopflow.cli.api.current_session",
        lambda _cwd: _session(),
    )

    def run_worker(
        wave: str,
        flow: str,
        task: str,
        parent_session_id: str | None = None,
    ) -> Session:
        received.update(
            {
                "wave": wave,
                "flow": flow,
                "task": task,
                "parent_session_id": parent_session_id,
            }
        )
        return _worker_launch_response()

    monkeypatch.setattr("loopflow.cli.api.run_worker", run_worker)

    result = CliRunner().invoke(
        app,
        ["worker", "run", "--flow", "implement", "--task", "Add the endpoint"],
    )

    assert result.exit_code == 0
    assert received == {
        "wave": "abc-123",
        "flow": "implement",
        "task": "Add the endpoint",
        "parent_session_id": "terminal-session-agent",
    }


def test_worker_run_rejects_worker_caller(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("LFD_SESSION_ID", "terminal-session-child")
    monkeypatch.setattr(
        "loopflow.cli.api.get_session",
        lambda _session_id: _session(
            id="terminal-session-child",
            run_id="run-1",
            use="worker",
        ),
    )
    monkeypatch.setattr(
        "loopflow.cli.api.run_worker",
        lambda *args, **kwargs: (_ for _ in ()).throw(
            LoopflowError("worker sessions cannot launch worker sessions")
        ),
    )

    result = CliRunner().invoke(
        app,
        ["worker", "run", "reduce", "--flow", "implement", "--task", "Add the endpoint"],
    )

    assert result.exit_code == 1
    assert "worker sessions cannot launch worker sessions" in result.stderr


def test_attach_command_execs_tmux(monkeypatch: pytest.MonkeyPatch) -> None:
    executed: dict[str, object] = {}

    def fake_execvp(file: str, args: list[str]) -> None:
        executed["file"] = file
        executed["args"] = args

    monkeypatch.setattr(
        "loopflow.cli.api.attach_session",
        lambda _session_id: SessionConnectionInfo(
            kind="tmux",
            session_name="lfq-terminal-session-child",
            host="localhost",
            cwd="/tmp/repo",
            status="running",
        ),
    )
    monkeypatch.setattr("loopflow.cli.shutil.which", lambda _name: "/usr/bin/tmux")
    monkeypatch.setattr("loopflow.cli.os.execvp", fake_execvp)

    result = CliRunner().invoke(app, ["attach", "terminal-session-child"])

    assert result.exit_code == 0
    assert executed["file"] == "tmux"
    assert executed["args"] == ["tmux", "attach", "-t", "lfq-terminal-session-child"]


def test_attach_command_errors_cleanly(monkeypatch: pytest.MonkeyPatch) -> None:
    def fail_attach(_session_id: str) -> SessionConnectionInfo:
        raise LoopflowError("terminal session is not tmux-backed")

    monkeypatch.setattr("loopflow.cli.api.attach_session", fail_attach)

    result = CliRunner().invoke(app, ["attach", "terminal-session-child"])

    assert result.exit_code == 1
    assert "terminal session is not tmux-backed" in result.stderr


def test_auth_status_table_shows_active_and_none_states() -> None:
    statuses = [
        AuthProviderStatus.model_validate(AUTH_PROVIDER_ACTIVE),
        AuthProviderStatus.model_validate(AUTH_PROVIDER_NONE),
    ]
    console = Console(record=True, width=220)
    console.print(_auth_status_table(statuses))
    rendered = console.export_text()

    assert "GitHub" in rendered
    assert "@jackdanger" in rendered
    assert "OpenCode Zen" in rendered
    assert "not connected" in rendered


def test_auth_status_table_shows_expiry_details() -> None:
    statuses = [AuthProviderStatus.model_validate(AUTH_PROVIDER_ACTIVE_WITH_TIMESTAMPS)]
    console = Console(record=True, width=220)
    console.print(_auth_status_table(statuses))
    rendered = console.export_text()

    assert "Claude" in rendered
    assert "jack@anthropic.com" in rendered
    assert "expires" in rendered
    assert "refresh in" in rendered


def test_auth_status_table_shows_new_pm_providers() -> None:
    statuses = [
        AuthProviderStatus.model_validate({"provider": "asana", "status": "active"}),
    ]
    console = Console(record=True, width=220)
    console.print(_auth_status_table(statuses))
    rendered = console.export_text()

    assert "Asana" in rendered


def test_auth_status_table_does_not_mark_pm_api_keys_as_metered() -> None:
    statuses = [
        AuthProviderStatus.model_validate(
            {"provider": "asana", "status": "active", "credential_type": "apikey"}
        )
    ]
    console = Console(record=True, width=220)
    console.print(_auth_status_table(statuses))
    rendered = console.export_text()

    assert "Asana" in rendered
    assert "pay-per-token" not in rendered


def test_pm_providers_do_not_support_api_key_configure() -> None:
    assert _provider_api_key_config("asana") is None
    assert _pm_oauth_configure_error("asana") == (
        "Asana requires OAuth. Run 'lf op auth asana' to connect."
    )


def test_auth_poll_timeout_uses_provider_expiry_when_present() -> None:
    assert _auth_poll_timeout_seconds(900) == 900


def test_auth_poll_timeout_falls_back_for_missing_or_invalid_expiry() -> None:
    assert _auth_poll_timeout_seconds(None) == 180
    assert _auth_poll_timeout_seconds(0) == 180


def test_extract_authorization_code_accepts_raw_code() -> None:
    assert _extract_authorization_code("abc123") == "abc123"


def test_extract_authorization_code_parses_redirect_url() -> None:
    assert (
        _extract_authorization_code("urn:ietf:wg:oauth:2.0:oob?code=abc123&state=ignored")
        == "abc123"
    )


def test_repo_table_shows_registration_columns() -> None:
    repo = Repo.model_validate(REPO_MINIMAL)
    console = Console(record=True, width=220)
    console.print(_repo_table([repo]))
    rendered = console.export_text()

    assert "repo_id" in rendered
    assert "registered" in rendered
    assert "added_at" in rendered
    assert "yes" in rendered


def test_auth_status_shows_refreshing_soon_when_past_refresh_time() -> None:
    status = AuthProviderStatus.model_validate(
        {
            "provider": "claude",
            "status": "active",
            "login": "jack@anthropic.com",
            "expires_at": "2030-01-01T04:00:00Z",
            "next_refresh_at": "2020-01-01T00:00:00Z",
        }
    )
    details = _status_details(status)
    assert "refreshing soon" in details


def test_auth_status_no_refresh_when_no_next_refresh_at() -> None:
    status = AuthProviderStatus.model_validate(AUTH_PROVIDER_ACTIVE)
    details = _status_details(status)
    assert "refresh" not in details


def test_split_repo_slug_parses_owner_repo() -> None:
    assert _split_repo_slug("loopflowstudio/loopflow") == ("loopflowstudio", "loopflow")


# Providers table


def test_providers_table_renders_providers() -> None:
    providers = [
        ProviderInfo.model_validate(PROVIDER_INFO_MINIMAL),
        ProviderInfo.model_validate(PROVIDER_INFO_FULL),
    ]
    console = Console(record=True, width=220)
    console.print(_providers_table(providers))
    rendered = console.export_text()

    assert "Codex" in rendered
    assert "OpenCode Zen" in rendered
    assert "subscription" in rendered
    assert "per_token" in rendered
    assert "GPT-5.1 Codex" in rendered
    assert "Kimi K2.5" in rendered
    assert "\u2713 active" in rendered
    assert "\u2717 none" in rendered
