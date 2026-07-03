from __future__ import annotations

from collections.abc import Iterator
from typing import Any, Optional

from .client import Client
from .models import (
    AuthFlow,
    AuthProviderStatus,
    CommitEntry,
    Conversation,
    ConversationEventEnvelope,
    CostRates,
    ModelInfo,
    ProviderInfo,
    PullRequest,
    Repo,
    Run,
    Session,
    SessionConnectionInfo,
    TokenTotals,
    Trigger,
    UsageSummary,
    UsageSummaryGroup,
    Wave,
    WaveAgentTree,
    WaveCron,
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
    crons: Optional[list[dict[str, str]]] = None,
    direction: Optional[list[str]] = None,
    area: Optional[list[str]] = None,
    status: Optional[str] = None,
    goal: Optional[str] = None,
) -> Wave:
    return _client().create_wave(
        name,
        repo,
        flow=flow,
        goal=goal,
        crons=crons,
        direction=direction,
        area=area,
        status=status,
    )


def update_wave(
    name_or_id: str,
    flow: Optional[str] = None,
    crons: Optional[list[dict[str, str]]] = None,
    direction: Optional[list[str]] = None,
    area: Optional[list[str]] = None,
    status: Optional[str] = None,
    goal: Optional[str] = None,
) -> Wave:
    return _client().update_wave(
        name_or_id,
        flow=flow,
        goal=goal,
        crons=crons,
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
    goal: Optional[str] = None,
) -> Session:
    return _client().run_wave(
        name_or_id,
        flow=flow,
        goal=goal,
        direction=direction,
        area=area,
    )


def ensure_wave_agent(name_or_id: str) -> Session:
    return _client().ensure_wave_agent(name_or_id)


def get_wave_agent_tree(name_or_id: str, active_only: bool = True) -> WaveAgentTree:
    return _client().get_wave_agent_tree(name_or_id, active_only=active_only)


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


def runs(
    wave_id: Optional[str] = None,
    repo: Optional[str] = None,
    limit: Optional[int] = None,
) -> list[Run]:
    return _client().runs(wave_id=wave_id, repo=repo, limit=limit)


def wave_logs(name_or_id: str) -> Iterator[str]:
    return _client().wave_logs(name_or_id)


def run_worker(
    name_or_id: str,
    flow: str,
    task: str,
    parent_session_id: Optional[str] = None,
) -> Session:
    return _client().run_worker(
        name_or_id,
        flow,
        task,
        parent_session_id=parent_session_id,
    )


def list_sessions(
    wave_id: Optional[str] = None,
    parent_session_id: Optional[str] = None,
    use: Optional[str] = None,
    active_only: bool = True,
) -> list[Session]:
    return _client().list_sessions(
        wave_id=wave_id,
        parent_session_id=parent_session_id,
        use=use,
        active_only=active_only,
    )


def get_session(session_id: str) -> Session:
    return _client().get_session(session_id)


def current_session(cwd: str) -> Optional[Session]:
    return _client().current_session(cwd)


def attach_session(session_id: str) -> SessionConnectionInfo:
    return _client().attach_session(session_id)


def list_attention(status: Optional[str] = None) -> list[dict[str, Any]]:
    return _client().list_attention(status=status)


def send_conversation_input(session_id: str, content: str) -> Conversation:
    return _client().send_conversation_input(session_id, content)


def stream_conversation_events(
    session_id: str,
    after_seq: Optional[int] = None,
    timeout: float = 60.0,
) -> Iterator[ConversationEventEnvelope]:
    return _client().stream_conversation_events(
        session_id,
        after_seq=after_seq,
        timeout=timeout,
    )


__all__ = [
    "Client",
    "AuthFlow",
    "AuthProviderStatus",
    "CommitEntry",
    "Conversation",
    "CostRates",
    "ModelInfo",
    "PullRequest",
    "ProviderInfo",
    "Repo",
    "Session",
    "ConversationEventEnvelope",
    "SessionConnectionInfo",
    "Trigger",
    "TokenTotals",
    "UsageSummary",
    "UsageSummaryGroup",
    "Wave",
    "WaveAgentTree",
    "WaveCron",
    "Run",
    "health",
    "status",
    "auth_status",
    "start_auth",
    "complete_auth",
    "disconnect_auth",
    "configure_api_key",
    "providers",
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
    "ensure_wave_agent",
    "get_wave_agent_tree",
    "add_trigger",
    "remove_trigger",
    "stop_wave",
    "land_wave",
    "next_wave",
    "runs",
    "wave_logs",
    "run_worker",
    "list_sessions",
    "current_session",
    "get_session",
    "attach_session",
    "send_conversation_input",
    "stream_conversation_events",
]
