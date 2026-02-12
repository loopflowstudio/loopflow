from __future__ import annotations

from collections.abc import Iterator
from typing import Any, Optional

from .client import Client
from .models import CommitEntry, PullRequest, Stimulus, Wave, WaveRun

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
    stimulus: Optional[Stimulus] = None,
    status: Optional[str] = None,
) -> Wave:
    return _client().update_wave(
        name_or_id,
        flow=flow,
        direction=direction,
        area=area,
        stimulus=stimulus,
        status=status,
    )


def delete_wave(name_or_id: str) -> None:
    _client().delete_wave(name_or_id)


def run_wave(
    name_or_id: str,
    flow: Optional[str] = None,
    direction: Optional[list[str]] = None,
    area: Optional[list[str]] = None,
    stimulus: Optional[Stimulus] = None,
) -> dict[str, Any]:
    return _client().run_wave(
        name_or_id,
        flow=flow,
        direction=direction,
        area=area,
        stimulus=stimulus,
    )


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


__all__ = [
    "Client",
    "CommitEntry",
    "PullRequest",
    "Stimulus",
    "Wave",
    "WaveRun",
    "health",
    "status",
    "waves",
    "wave",
    "create_wave",
    "update_wave",
    "delete_wave",
    "run_wave",
    "stop_wave",
    "land_wave",
    "next_wave",
    "wave_runs",
    "wave_logs",
]
