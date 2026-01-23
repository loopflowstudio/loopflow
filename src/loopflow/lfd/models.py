"""Data structures for lfd daemon."""

import json
from datetime import datetime
from enum import Enum
from pathlib import Path

from pydantic import BaseModel, ConfigDict, Field, field_validator

# Re-export Session and SessionStatus from lf.models for backwards compatibility
from loopflow.lf.models import Session as Session
from loopflow.lf.models import SessionStatus as SessionStatus


def area_to_slug(area: str) -> str:
    """Convert area to slug: 'swift/' -> 'swift', '.' -> 'root'."""
    if area == ".":
        return "root"
    return area.rstrip("/").split("/")[-1].lower()


# Shared base model


class LfdModel(BaseModel):
    """Base model for lfd data structures."""

    model_config = ConfigDict(
        extra="forbid",
        validate_assignment=True,
    )


# Agent: an AI coding agent


class AgentStatus(str, Enum):
    """Runtime status of an agent."""

    IDLE = "idle"
    RUNNING = "running"
    WAITING = "waiting"
    ERROR = "error"


class MergeMode(str, Enum):
    """How iteration branches merge to main."""

    PR = "pr"
    LAND = "land"


class Agent(LfdModel):
    """An AI coding agent.

    Activation modes (derived from config):
    - Continuous (default): runs when started until stopped or PR limit
    - Watch: runs when watch_paths change on main
    - Scheduled: runs on cron schedule
    """

    id: str
    repo: Path
    flow: str
    voice: list[str] = Field(min_length=1)
    area: list[str] = Field(min_length=1)

    status: AgentStatus = AgentStatus.IDLE
    iteration: int = 0

    main_branch: str = ""
    pr_limit: int = Field(default=5, ge=1)
    merge_mode: MergeMode = MergeMode.PR

    pid: int | None = None
    created_at: datetime = Field(default_factory=datetime.now)

    # Activation config (determines mode)
    watch_paths: str | None = None
    cron: str | None = None
    last_main_sha: str | None = None

    @field_validator("voice", mode="before")
    @classmethod
    def normalize_voice(cls, v):
        if isinstance(v, str):
            return [v]
        return v

    @field_validator("area", mode="before")
    @classmethod
    def normalize_area(cls, v):
        if isinstance(v, str):
            return [v]
        return v

    def short_id(self) -> str:
        return self.id[:7]

    @property
    def area_slug(self) -> str:
        return area_to_slug(self.area[0])

    @property
    def mode(self) -> str:
        """Return the activation mode: 'watch', 'cron', or 'loop'."""
        if self.watch_paths:
            return "watch"
        if self.cron:
            return "cron"
        return "loop"

    @property
    def voice_display(self) -> str:
        return ", ".join(self.voice)

    @property
    def area_display(self) -> str:
        return ", ".join(self.area)


def agent_from_row(row: dict) -> Agent:
    """Convert database row to Agent."""
    voice_str = row.get("voice")
    voice = json.loads(voice_str) if voice_str else ["default"]

    area_str = row.get("area")
    area = json.loads(area_str) if area_str else ["."]

    merge_mode_str = row.get("merge_mode", "pr")
    if merge_mode_str == "auto":
        merge_mode_str = "pr"

    return Agent(
        id=row["id"],
        repo=Path(row["repo"]),
        flow=row["flow"],
        voice=voice,
        area=area,
        status=AgentStatus(row["status"]),
        iteration=row.get("iteration", 0),
        main_branch=row.get("main_branch", ""),
        pr_limit=row.get("pr_limit", 5),
        merge_mode=MergeMode(merge_mode_str),
        pid=row.get("pid"),
        created_at=datetime.fromisoformat(row["created_at"]),
        watch_paths=row.get("watch_paths"),
        cron=row.get("cron"),
        last_main_sha=row.get("last_main_sha"),
    )


# Run: an execution instance of a Flow


class RunStatus(str, Enum):
    """Status of a Run execution."""

    PENDING = "pending"
    RUNNING = "running"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


class Run(LfdModel):
    """An execution instance of a Flow, spawned by an Agent."""

    id: str
    agent: str | None = None

    flow: str
    voice: list[str] = Field(min_length=1)
    area: list[str] = Field(min_length=1)
    repo: Path

    status: RunStatus = RunStatus.PENDING
    iteration: int = 0

    worktree: str | None = None
    branch: str | None = None
    current_step: str | None = None
    error: str | None = None
    pr_url: str | None = None

    started_at: datetime | None = None
    ended_at: datetime | None = None
    created_at: datetime = Field(default_factory=datetime.now)
