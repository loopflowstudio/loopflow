"""Tests for loopflow.cli table rendering."""

from __future__ import annotations

from rich.console import Console

from conftest import AUTH_PROVIDER_ACTIVE, AUTH_PROVIDER_NONE, WAVE_FULL, WAVE_MINIMAL

from loopflow.cli import _auth_status_table, _wave_table
from loopflow.models import AuthProviderStatus, Wave


def _render_table(wave: Wave) -> str:
    console = Console(record=True, width=220)
    console.print(_wave_table([wave]))
    return console.export_text()


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


def test_wave_table_falls_back_to_wave_branch() -> None:
    payload = dict(WAVE_MINIMAL)
    payload["remote_branch"] = "wave/fallback"
    payload["local_worktree"] = "/tmp/fallback-wt"
    wave = Wave.model_validate(payload)
    rendered = _render_table(wave)

    assert "wave/fallback" in rendered
    assert "/tmp/fallback-wt" in rendered


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
    assert "Codex" in rendered
    assert "not connected" in rendered
