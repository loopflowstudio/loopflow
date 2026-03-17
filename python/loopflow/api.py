from __future__ import annotations

from collections.abc import Iterator
from typing import Any, Optional

from .client import Client
from .models import (
    AuthFlow,
    AuthProviderStatus,
    CommitEntry,
    CostRates,
    ModelInfo,
    ProviderInfo,
    PullRequest,
    Repo,
    Session,
    SessionConfig,
    SessionEventEnvelope,
    TokenTotals,
    Trigger,
    UsageSummary,
    UsageSummaryGroup,
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


def complete_auth(provider: str, code: str) -> None:
    _client().complete_auth(provider, code)


def disconnect_auth(provider: str) -> AuthProviderStatus:
    return _client().disconnect_auth(provider)


def configure_api_key(
    provider: str,
    api_key: str,
) -> AuthProviderStatus:
    return _client().configure_api_key(provider, api_key)


def providers() -> list[ProviderInfo]:
    return _client().providers()


def revoke_connection_tokens(
    prefix: Optional[str] = None,
    revoke_all: bool = False,
) -> int:
    return _client().revoke_connection_tokens(prefix=prefix, revoke_all=revoke_all)


def usage_summary(
    group_by: str = "wave",
    wave: Optional[str] = None,
    flow: Optional[str] = None,
    step: Optional[str] = None,
    model: Optional[str] = None,
    source: Optional[str] = None,
    from_: Optional[str] = None,
    to_: Optional[str] = None,
) -> UsageSummary:
    return _client().usage_summary(
        group_by=group_by,
        wave=wave,
        flow=flow,
        step=step,
        model=model,
        source=source,
        from_=from_,
        to_=to_,
    )


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


def list_repos() -> list[Repo]:
    return _client().list_repos()


def add_repo(path: str) -> Repo:
    return _client().add_repo(path)


def remove_repo(path: str) -> None:
    _client().remove_repo(path)


def add_child(owner: str, repo: str, child_owner: str, child_repo: str) -> None:
    _client().add_child(owner, repo, child_owner, child_repo)


def remove_child(owner: str, repo: str, child_owner: str, child_repo: str) -> None:
    _client().remove_child(owner, repo, child_owner, child_repo)


def list_children(owner: str, repo: str) -> list[Repo]:
    return _client().list_children(owner, repo)


def list_parents(owner: str, repo: str) -> list[Repo]:
    return _client().list_parents(owner, repo)


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


def add_trigger(
    name_or_id: str,
    signal: str,
    flow: Optional[str] = None,
    source_wave_id: Optional[str] = None,
    max_iterations: Optional[int] = None,
) -> dict[str, Any]:
    return _client().add_trigger(
        name_or_id,
        signal,
        flow=flow,
        source_wave_id=source_wave_id,
        max_iterations=max_iterations,
    )


def remove_trigger(name_or_id: str, trigger_id: str) -> dict[str, Any]:
    return _client().remove_trigger(name_or_id, trigger_id)


def stop_wave(name_or_id: str) -> dict[str, Any]:
    return _client().stop_wave(name_or_id)


def land_wave(
    name_or_id: str,
    strict: Optional[bool] = None,
    local: Optional[bool] = None,
    create_pr: Optional[bool] = None,
    worktree: Optional[str] = None,
) -> dict[str, Any]:
    return _client().land_wave(
        name_or_id,
        strict=strict,
        local=local,
        create_pr=create_pr,
        worktree=worktree,
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
    "CommitEntry",
    "CostRates",
    "ModelInfo",
    "PullRequest",
    "ProviderInfo",
    "Repo",
    "Session",
    "SessionConfig",
    "SessionEventEnvelope",
    "Trigger",
    "TokenTotals",
    "UsageSummary",
    "UsageSummaryGroup",
    "Wave",
    "WaveRun",
    "health",
    "status",
    "auth_status",
    "start_auth",
    "complete_auth",
    "disconnect_auth",
    "configure_api_key",
    "providers",
    "revoke_connection_tokens",
    "usage_summary",
    "waves",
    "wave",
    "create_wave",
    "update_wave",
    "delete_wave",
    "list_repos",
    "add_repo",
    "remove_repo",
    "add_child",
    "remove_child",
    "list_children",
    "list_parents",
    "run_wave",
    "add_trigger",
    "remove_trigger",
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
