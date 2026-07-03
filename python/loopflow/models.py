from __future__ import annotations

from datetime import datetime
from typing import Any, Optional

from pydantic import BaseModel, Field, field_validator


class Trigger(BaseModel):
    id: Optional[str] = None
    signal: str
    source_wave_id: Optional[str] = None
    flow: Optional[str] = None
    max_iterations: Optional[int] = None


class WaveCron(BaseModel):
    id: Optional[str] = None
    flow: str
    schedule: str
    last_triggered_at: Optional[int] = None
    created_at: Optional[datetime] = None


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
    id: str
    name: str
    repo: str
    mode: str = "loop"
    primary_flow: str = "ship-roadmap"
    goal: str
    metrics: list[str]
    workers: int = 1
    direction: list[str]
    area: list[str]
    triggers: list[Trigger] = Field(default_factory=list)
    crons: list[WaveCron] = Field(default_factory=list)
    status: str
    iteration: int
    local_worktree: Optional[str] = None
    remote_branch: Optional[str] = None
    commits: list[CommitEntry] = Field(default_factory=list)
    diff_stat: Optional[str] = None
    flow_steps: list[FlowStep] = Field(default_factory=list)
    active_run: Optional[Run] = None
    created_at: Optional[datetime] = None

    branch: Optional[str] = None
    pr_url: Optional[str] = None
    pr_state: Optional[str] = None

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


class ConversationConfig(BaseModel):
    agent: Optional[str] = None
    cwd: Optional[str] = None
    system_prompt: Optional[str] = None
    max_turns: Optional[int] = None
    yolo_mode: bool = False


class Conversation(BaseModel):
    id: str
    object: str = "conversation"
    harness: str
    status: str
    run_id: Optional[str] = None
    provider_session_id: Optional[str] = None
    config: ConversationConfig = Field(default_factory=ConversationConfig)
    input_supported: bool = False
    created_at: Optional[datetime] = None
    ended_at: Optional[datetime] = None


class ConversationEventEnvelope(BaseModel):
    seq: Optional[int] = None
    event: dict[str, Any]


class TokenTotals(BaseModel):
    input: int = 0
    output: int = 0
    reasoning: int = 0
    cache_read: int = 0
    cache_write: int = 0


class UsageSummaryGroup(BaseModel):
    key: str
    tokens: TokenTotals = Field(default_factory=TokenTotals)
    sessions: int = 0
    turns: int = 0


class UsageSummary(BaseModel):
    object: str = "usage_summary"
    group_by: str
    from_time: Optional[str] = Field(None, alias="from")
    to_time: Optional[str] = Field(None, alias="to")
    groups: list[UsageSummaryGroup] = Field(default_factory=list)
