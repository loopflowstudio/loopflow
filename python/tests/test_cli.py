"""Tests for loopflow.cli table rendering."""

from __future__ import annotations

from rich.console import Console

from conftest import (
    AUTH_PROVIDER_ACTIVE,
    AUTH_PROVIDER_ACTIVE_WITH_TIMESTAMPS,
    AUTH_PROVIDER_NONE,
    REPO_MINIMAL,
    WAVE_FULL,
    WAVE_MINIMAL,
)

from loopflow.cli import _auth_status_table, _repo_table, _split_repo_slug, _status_details, _wave_table
from loopflow.models import AuthProviderStatus, Repo, Wave


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


def test_auth_status_table_shows_expiry_details() -> None:
    statuses = [AuthProviderStatus.model_validate(AUTH_PROVIDER_ACTIVE_WITH_TIMESTAMPS)]
    console = Console(record=True, width=220)
    console.print(_auth_status_table(statuses))
    rendered = console.export_text()

    assert "Claude" in rendered
    assert "jack@anthropic.com" in rendered
    assert "expires" in rendered
    assert "refresh in" in rendered


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
