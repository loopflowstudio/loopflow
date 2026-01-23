"""Data structures for lfd daemon."""

import json
from datetime import datetime
from enum import Enum
from pathlib import Path

from pydantic import BaseModel, ConfigDict, Field, field_validator


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


class AgentMode(str, Enum):
    """Activation mode of an agent."""

    LOOP = "loop"
    WATCH = "watch"
    CRON = "cron"


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

    Activation modes:
    - Loop (default): runs when started until stopped or PR limit
    - Watch: runs when watch_paths change on main
    - Cron: runs on cron schedule
    """

    id: str
    repo: Path
    flow: str
    voice: list[str] = Field(min_length=1)
    area: list[str] = Field(min_length=1)

    mode: AgentMode = AgentMode.LOOP
    status: AgentStatus = AgentStatus.IDLE
    iteration: int = 0

    main_branch: str = ""
    pr_limit: int = Field(default=5, ge=1)
    merge_mode: MergeMode = MergeMode.PR

    pid: int | None = None
    created_at: datetime = Field(default_factory=datetime.now)

    # Trigger config (for watch/cron modes)
    watch_paths: str | None = None
    cron: str | None = None
    last_main_sha: str | None = None

    # Circuit breaker
    consecutive_failures: int = 0

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

    # Read mode from DB, with fallback for pre-migration rows
    mode_str = row.get("mode")
    if mode_str:
        mode = AgentMode(mode_str)
    elif row.get("watch_paths"):
        mode = AgentMode.WATCH
    elif row.get("cron"):
        mode = AgentMode.CRON
    else:
        mode = AgentMode.LOOP

    return Agent(
        id=row["id"],
        repo=Path(row["repo"]),
        flow=row["flow"],
        voice=voice,
        area=area,
        mode=mode,
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
        consecutive_failures=row.get("consecutive_failures", 0),
    )


# FlowRun: an execution instance of a Flow


class FlowRunStatus(str, Enum):
    """Status of a FlowRun execution."""

    PENDING = "pending"
    RUNNING = "running"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


class FlowRun(LfdModel):
    """An execution instance of a Flow, spawned by an Agent."""

    id: str
    agent_id: str | None = None

    flow: str
    voice: list[str] = Field(min_length=1)
    area: list[str] = Field(min_length=1)
    repo: Path

    status: FlowRunStatus = FlowRunStatus.PENDING
    iteration: int = 0

    worktree: str | None = None
    branch: str | None = None
    current_step: str | None = None
    error: str | None = None
    pr_url: str | None = None

    started_at: datetime | None = None
    ended_at: datetime | None = None
    created_at: datetime = Field(default_factory=datetime.now)


# StepRun: an execution of a single step


class StepRunStatus(str, Enum):
    """Status of a StepRun execution."""

    RUNNING = "running"
    WAITING = "waiting"
    COMPLETED = "completed"
    FAILED = "failed"


class StepRun(LfdModel):
    """An execution of a single step.

    Can belong to a FlowRun (agent-spawned) or be standalone (interactive).
    """

    id: str
    step: str
    repo: str
    worktree: str

    flow_run_id: str | None = None
    agent_id: str | None = None

    status: StepRunStatus = StepRunStatus.RUNNING
    started_at: datetime = Field(default_factory=datetime.now)
    ended_at: datetime | None = None

    pid: int | None = None
    model: str = "claude-code"
    run_mode: str = "auto"

    def to_dict(self) -> dict:
        return {
            "id": self.id,
            "step": self.step,
            "repo": self.repo,
            "worktree": self.worktree,
            "flow_run_id": self.flow_run_id,
            "agent_id": self.agent_id,
            "status": self.status.value,
            "started_at": self.started_at.isoformat(),
            "ended_at": self.ended_at.isoformat() if self.ended_at else None,
            "pid": self.pid,
            "model": self.model,
            "run_mode": self.run_mode,
        }

    @classmethod
    def from_dict(cls, data: dict) -> "StepRun":
        return cls(
            id=data["id"],
            step=data["step"],
            repo=data["repo"],
            worktree=data["worktree"],
            flow_run_id=data.get("flow_run_id"),
            agent_id=data.get("agent_id"),
            status=StepRunStatus(data["status"]),
            started_at=datetime.fromisoformat(data["started_at"]),
            ended_at=datetime.fromisoformat(data["ended_at"]) if data.get("ended_at") else None,
            pid=data.get("pid"),
            model=data.get("model", "claude-code"),
            run_mode=data.get("run_mode", "auto"),
        )
