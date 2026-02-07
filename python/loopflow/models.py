from __future__ import annotations

from datetime import datetime
from typing import Optional

from pydantic import BaseModel, Field


class Stimulus(BaseModel):
    kind: str
    cron: Optional[str] = None


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


class Wave(BaseModel):
    id: str
    name: str
    repo: str
    flow: str
    direction: list[str]
    area: list[str]
    stimulus: Stimulus
    status: str
    iteration: int
    active_run: Optional[WaveRun] = None
    created_at: Optional[datetime] = None

    branch: Optional[str] = None
    pr_url: Optional[str] = None
    pr_state: Optional[str] = None
