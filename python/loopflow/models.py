from __future__ import annotations

from datetime import datetime
from typing import Any, Optional

from pydantic import BaseModel, Field, field_validator


class PullRequest(BaseModel):
    url: str
    number: Optional[int] = None
    state: Optional[str] = None
    title: Optional[str] = None
    branch: Optional[str] = None


class Run(BaseModel):
    id: str
    wave_id: str
    task: Optional[str] = None
    iteration: int
    step_index: int
    status: str
    local_worktree: str
    remote_branch: str
    pr: Optional[PullRequest] = None
    started_at: Optional[datetime] = None
    ended_at: Optional[datetime] = None
    error: Optional[str] = None
    flow_parents: list[str] = Field(default_factory=list)


class CommitEntry(BaseModel):
    sha: str
    message: str


class FlowStep(BaseModel):
    type: str
    name: str

    @classmethod
    def from_raw(cls, value: Any) -> "FlowStep":
        if isinstance(value, FlowStep):
            return value
        if isinstance(value, dict):
            return cls.model_validate(value)
        if isinstance(value, str):
            for prefix in ("op:",):
                if value.startswith(prefix):
                    return cls(type="op", name=value.split(":", 1)[1].strip())
            for marker, step_type in (
                ("[branch]", "branch"),
                ("[fork]", "fork"),
                ("[loop]", "loop"),
            ):
                if value == marker:
                    return cls(type=step_type, name=step_type)
            return cls(type="step", name=value)
        raise TypeError(f"Unsupported flow step value: {value!r}")


class Wave(BaseModel):
    """Wave wire type. A wave targets exactly one repo, so its execution surface
    (repo/iteration plus the git + PR snapshot) is carried inline. No field
    defaults — every field the server always emits is required, mirroring Rust's
    `WaveDto` (absent → parse error). Optional fields carry `None` for absent,
    never a value default."""

    id: str
    name: str
    primary_flow: str
    goal: str
    metrics: list[str]
    workers: int
    direction: list[str]
    area: list[str]
    status: str
    repo: str
    iteration: int
    commits: list[CommitEntry]
    open_pr_count: int
    stack_count: int
    flow_steps: list[FlowStep]
    parent_wave_id: Optional[str]
    local_worktree: Optional[str] = None
    remote_branch: Optional[str] = None
    diff_stat: Optional[str] = None
    active_run: Optional[Run] = None
    pr: Optional[PullRequest] = None
    created_at: Optional[datetime] = None

    @field_validator("flow_steps", mode="before")
    @classmethod
    def _parse_flow_steps(cls, value: Any) -> list[FlowStep]:
        if value in (None, ""):
            return []
        if not isinstance(value, list):
            return []
        return [FlowStep.from_raw(item) for item in value]


class Session(BaseModel):
    id: str
    object: str
    wave_id: str
    run_id: Optional[str]
    parent_session_id: Optional[str]
    session_use: str = Field(alias="use")
    step: str
    agent: str
    cwd: str
    argv: list[str]
    env: dict[str, str]
    source: str
    tmux_name: str
    status: str
    created_at: datetime
    attached_at: Optional[datetime]
    started_at: Optional[datetime]
    completed_at: Optional[datetime]


class SessionConnectionInfo(BaseModel):
    kind: str
    session_name: str
    host: str
    cwd: str
    status: str


class WaveAgentTreeSession(BaseModel):
    session: Session
    connection: Optional[SessionConnectionInfo]


class WaveAgentTree(BaseModel):
    object: str
    id: str
    wave: Wave
    child_waves: list[Wave]
    sessions: list[WaveAgentTreeSession]


class AuthProviderStatus(BaseModel):
    provider: str
    status: str
    login: Optional[str] = None
    expires_at: Optional[datetime] = None
    next_refresh_at: Optional[datetime] = None
    credential_type: Optional[str] = None


class AuthFlow(BaseModel):
    provider: str
    verification_uri: str
    verification_uri_complete: Optional[str] = None
    user_code: Optional[str] = None
    expires_in: Optional[int] = None


class CostRates(BaseModel):
    input_per_mtok: float
    output_per_mtok: float
    cache_read_per_mtok: Optional[float] = None
    cache_write_per_mtok: Optional[float] = None


class ModelInfo(BaseModel):
    id: str
    display_name: str
    provider: str
    cost_rates: Optional[CostRates] = None


class ProviderInfo(BaseModel):
    provider: str
    auth_status: str
    login: Optional[str] = None
    billing: str
    models: list[ModelInfo] = Field(default_factory=list)


class Repo(BaseModel):
    path: str
    name: str
    repo_id: str
    wave_count: int
    registered: bool
    added_at: Optional[datetime] = None
