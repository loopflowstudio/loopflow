from __future__ import annotations

from datetime import datetime
from typing import Any, Optional

from pydantic import BaseModel, Field, field_validator


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
            for marker, skill_type in (
                ("[branch]", "branch"),
                ("[fork]", "fork"),
                ("[loop]", "loop"),
            ):
                if value == marker:
                    return cls(type=skill_type, name=skill_type)
            return cls(type="skill", name=value)
        raise TypeError(f"Unsupported flow skill value: {value!r}")


class Wave(BaseModel):
    """Wave wire type for one repository control plane."""

    id: str
    object: str
    name: str
    goal: str
    metrics: list[str]
    task_capacity: int
    direction: list[str]
    area: list[str]
    agent: Optional[str] = None
    skill_agents: Optional[dict[str, str]] = None
    status: str
    repo: str
    iteration: int
    flow_steps: list[FlowStep]
    parent_wave_id: Optional[str]
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
    skill: str
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


class WaveAgentTreeSession(BaseModel):
    session: Session


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
