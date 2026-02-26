from __future__ import annotations

from collections.abc import Iterator
from typing import Any, Optional

from .client import Client
from .models import (
    AuthFlow,
    AuthProviderStatus,
    Chord,
    CommitEntry,
    PullRequest,
    Session,
    SessionConfig,
    SessionEventEnvelope,
    Stimulus,
    Wave,
    WaveRun,
)

_default_client: Optional[Client] = None


def _client() -> Client:
    global _default_client
    if _default_client is None:
        _default_client = Client()
    return _default_client


def health() -> dict[str, Any]:
    return _client().health()


def status() -> dict[str, Any]:
    return _client().status()


def auth_status(provider: Optional[str] = None) -> list[AuthProviderStatus] | AuthProviderStatus:
    return _client().auth_status(provider=provider)


def start_auth(provider: str) -> AuthFlow:
    return _client().start_auth(provider)


def disconnect_auth(provider: str) -> AuthProviderStatus:
    return _client().disconnect_auth(provider)


def waves(repo: Optional[str] = None) -> list[Wave]:
    return _client().waves(repo=repo)


def wave(name_or_id: str) -> Optional[Wave]:
    return _client().wave(name_or_id)


def create_wave(
    name: str,
    repo: str,
    flow: Optional[str] = None,
    direction: Optional[list[str]] = None,
    area: Optional[list[str]] = None,
) -> Wave:
    return _client().create_wave(name, repo, flow=flow, direction=direction, area=area)


def update_wave(
    name_or_id: str,
    flow: Optional[str] = None,
    direction: Optional[list[str]] = None,
    area: Optional[list[str]] = None,
    status: Optional[str] = None,
) -> Wave:
    return _client().update_wave(
        name_or_id,
        flow=flow,
        direction=direction,
        area=area,
        status=status,
    )


def delete_wave(name_or_id: str) -> None:
    _client().delete_wave(name_or_id)


def create_chord(name: str) -> Chord:
    return _client().create_chord(name)


def list_chords() -> list[Chord]:
    return _client().list_chords()


def get_chord(chord_id: str) -> Optional[Chord]:
    return _client().get_chord(chord_id)


def delete_chord(chord_id: str) -> None:
    _client().delete_chord(chord_id)


def add_chord_member(chord_id: str, wave_id: str) -> None:
    _client().add_chord_member(chord_id, wave_id)


def remove_chord_member(chord_id: str, wave_id: str) -> None:
    _client().remove_chord_member(chord_id, wave_id)


def list_chord_members(chord_id: str) -> list[Wave]:
    return _client().list_chord_members(chord_id)


def list_wave_chords(wave_id: str) -> list[Chord]:
    return _client().list_wave_chords(wave_id)


def run_wave(
    name_or_id: str,
    flow: Optional[str] = None,
    direction: Optional[list[str]] = None,
    area: Optional[list[str]] = None,
) -> dict[str, Any]:
    return _client().run_wave(
        name_or_id,
        flow=flow,
        direction=direction,
        area=area,
    )


def add_stimulus(
    name_or_id: str,
    kind: str,
    cron: Optional[str] = None,
    source_wave_id: Optional[str] = None,
) -> dict[str, Any]:
    return _client().add_stimulus(
        name_or_id,
        kind,
        cron=cron,
        source_wave_id=source_wave_id,
    )


def remove_stimulus(name_or_id: str, stimulus_id: str) -> dict[str, Any]:
    return _client().remove_stimulus(name_or_id, stimulus_id)


def stop_wave(name_or_id: str) -> dict[str, Any]:
    return _client().stop_wave(name_or_id)


def land_wave(
    name_or_id: str,
    strict: Optional[bool] = None,
    local: Optional[bool] = None,
    create_pr: Optional[bool] = None,
    worktree: Optional[str] = None,
    lint: Optional[bool] = None,
) -> dict[str, Any]:
    return _client().land_wave(
        name_or_id,
        strict=strict,
        local=local,
        create_pr=create_pr,
        worktree=worktree,
        lint=lint,
    )


def next_wave(name_or_id: str) -> dict[str, Any]:
    return _client().next_wave(name_or_id)


def wave_runs(
    wave_id: Optional[str] = None,
    repo: Optional[str] = None,
    limit: Optional[int] = None,
) -> list[WaveRun]:
    return _client().wave_runs(wave_id=wave_id, repo=repo, limit=limit)


def wave_logs(name_or_id: str) -> Iterator[str]:
    return _client().wave_logs(name_or_id)


def create_session(
    harness: str,
    wave_run_id: Optional[str] = None,
    config: Optional[SessionConfig] = None,
) -> Session:
    return _client().create_session(harness, wave_run_id=wave_run_id, config=config)


def session(session_id: str) -> Optional[Session]:
    return _client().session(session_id)


def send_session_input(session_id: str, content: str) -> Session:
    return _client().send_session_input(session_id, content)


def stop_session(session_id: str) -> Session:
    return _client().stop_session(session_id)


def stream_session_events(
    session_id: str,
    after_seq: Optional[int] = None,
    timeout: float = 60.0,
) -> Iterator[SessionEventEnvelope]:
    return _client().stream_session_events(
        session_id,
        after_seq=after_seq,
        timeout=timeout,
    )


__all__ = [
    "Client",
    "AuthFlow",
    "AuthProviderStatus",
    "Chord",
    "CommitEntry",
    "PullRequest",
    "Session",
    "SessionConfig",
    "SessionEventEnvelope",
    "Stimulus",
    "Wave",
    "WaveRun",
    "health",
    "status",
    "auth_status",
    "start_auth",
    "disconnect_auth",
    "waves",
    "wave",
    "create_wave",
    "update_wave",
    "delete_wave",
    "create_chord",
    "list_chords",
    "get_chord",
    "delete_chord",
    "add_chord_member",
    "remove_chord_member",
    "list_chord_members",
    "list_wave_chords",
    "run_wave",
    "add_stimulus",
    "remove_stimulus",
    "stop_wave",
    "land_wave",
    "next_wave",
    "wave_runs",
    "wave_logs",
    "create_session",
    "session",
    "send_session_input",
    "stop_session",
    "stream_session_events",
]
