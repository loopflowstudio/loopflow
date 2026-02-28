from __future__ import annotations

from datetime import datetime
from typing import Any, Optional

from pydantic import BaseModel, Field


class Stimulus(BaseModel):
    id: Optional[str] = None
    kind: str
    source_wave_id: Optional[str] = None
    cron: Optional[str] = None
    max_iterations: Optional[int] = None


class PullRequest(BaseModel):
    url: str
    number: Optional[int] = None
    state: Optional[str] = None
    title: Optional[str] = None
    branch: Optional[str] = None


class WaveRun(BaseModel):
    id: str
    wave_id: str
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


class Wave(BaseModel):
    id: str
    name: str
    repo: str
    mode: str = "loop"
    primary_flow: str = "ship-roadmap"
    cron: Optional[str] = None
    direction: list[str]
    area: list[str]
    stimuli: list[Stimulus] = Field(default_factory=list)
    status: str
    iteration: int
    local_worktree: Optional[str] = None
    remote_branch: Optional[str] = None
    commits: list[CommitEntry] = Field(default_factory=list)
    diff_stat: Optional[str] = None
    flow_steps: list[str] = Field(default_factory=list)
    active_run: Optional[WaveRun] = None
    created_at: Optional[datetime] = None

    branch: Optional[str] = None
    pr_url: Optional[str] = None
    pr_state: Optional[str] = None


class AuthProviderStatus(BaseModel):
    provider: str
    status: str
    login: Optional[str] = None
    expires_at: Optional[datetime] = None
    next_refresh_at: Optional[datetime] = None


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


class Chord(BaseModel):
    id: str
    name: str
    is_default: bool = False
    created_at: Optional[datetime] = None


class Repo(BaseModel):
    path: str
    name: str
    repo_id: str
    wave_count: int
    registered: bool
    added_at: Optional[datetime] = None


class SessionConfig(BaseModel):
    agent: Optional[str] = None
    cwd: Optional[str] = None
    system_prompt: Optional[str] = None
    max_turns: Optional[int] = None
    yolo_mode: bool = False


class Session(BaseModel):
    id: str
    object: str = "session"
    harness: str
    status: str
    wave_run_id: Optional[str] = None
    provider_session_id: Optional[str] = None
    config: SessionConfig = Field(default_factory=SessionConfig)
    created_at: Optional[datetime] = None
    ended_at: Optional[datetime] = None


class SessionEventEnvelope(BaseModel):
    seq: Optional[int] = None
    event: dict[str, Any]
