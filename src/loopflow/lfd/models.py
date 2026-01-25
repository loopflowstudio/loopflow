"""Data structures for lfd daemon."""

import json
from dataclasses import dataclass
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Literal

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


@dataclass
class Stimulus:
    """Determines when an agent runs.

    Kinds:
    - once: single run (one-shot)
    - loop: continuously until stopped
    - watch: when files in area change on main
    - cron: on schedule
    """

    kind: Literal["once", "loop", "watch", "cron"]
    cron: str | None = None

    def __str__(self) -> str:
        if self.kind == "cron" and self.cron:
            return f"cron({self.cron})"
        return self.kind


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

    Stimulus types:
    - once: single run (one-shot)
    - loop: runs when started until stopped or PR limit
    - watch: runs when area changes on main
    - cron: runs on schedule

    area and goal are optional at creation time and validated at run-time.
    """

    id: str
    name: str  # unique name, used for worktree/branch naming
    repo: Path
    flow: str = "ship"  # default flow
    goal: list[str] | None = None  # optional, validated at run-time
    area: list[str] | None = None  # optional, validated at run-time

    stimulus: Stimulus = Field(default_factory=lambda: Stimulus("loop"))
    status: AgentStatus = AgentStatus.IDLE
    iteration: int = 0

    worktree: Path | None = None  # persistent worktree location
    branch: str | None = None  # current branch name
    pr_limit: int = Field(default=5, ge=1)
    merge_mode: MergeMode = MergeMode.PR

    pid: int | None = None
    created_at: datetime = Field(default_factory=datetime.now)

    # Watch state
    last_main_sha: str | None = None

    # Circuit breaker
    consecutive_failures: int = 0

    # Activation queue
    pending_activations: int = 0

    model_config = ConfigDict(
        extra="forbid",
        validate_assignment=True,
        arbitrary_types_allowed=True,
    )

    @field_validator("goal", mode="before")
    @classmethod
    def normalize_goal(cls, v):
        if v is None:
            return None
        if isinstance(v, str):
            return [v]
        return v

    @field_validator("area", mode="before")
    @classmethod
    def normalize_area(cls, v):
        if v is None:
            return None
        if isinstance(v, str):
            return [v]
        return v

    @field_validator("stimulus", mode="before")
    @classmethod
    def normalize_stimulus(cls, v):
        if isinstance(v, Stimulus):
            return v
        if isinstance(v, dict):
            return Stimulus(kind=v.get("kind", "loop"), cron=v.get("cron"))
        if isinstance(v, str):
            return Stimulus(kind=v)
        return v

    def short_id(self) -> str:
        return self.id[:7]

    @property
    def main_branch(self) -> str:
        """Main branch is {name}.main."""
        return f"{self.name}.main"

    @property
    def goal_display(self) -> str:
        if self.goal is None:
            return ""
        return ", ".join(self.goal)

    @property
    def area_display(self) -> str:
        if self.area is None:
            return ""
        return ", ".join(self.area)

    def is_configured(self) -> bool:
        """Check if agent has required config for running."""
        return self.area is not None


def agent_from_row(row: dict) -> Agent:
    """Convert database row to Agent."""
    goal_str = row.get("goal")
    goal = json.loads(goal_str) if goal_str else None

    area_str = row.get("area")
    area = json.loads(area_str) if area_str else None

    merge_mode_str = row.get("merge_mode", "pr")
    if merge_mode_str == "auto":
        merge_mode_str = "pr"

    # Build stimulus from DB columns
    stimulus = Stimulus(
        kind=row.get("stimulus_kind", "loop"),
        cron=row.get("stimulus_cron"),
    )

    worktree_str = row.get("worktree")
    worktree = Path(worktree_str) if worktree_str else None

    return Agent(
        id=row["id"],
        name=row["name"],
        repo=Path(row["repo"]),
        flow=row["flow"],
        goal=goal,
        area=area,
        stimulus=stimulus,
        status=AgentStatus(row["status"]),
        iteration=row.get("iteration", 0),
        worktree=worktree,
        branch=row.get("branch"),
        pr_limit=row.get("pr_limit", 5),
        merge_mode=MergeMode(merge_mode_str),
        pid=row.get("pid"),
        created_at=datetime.fromisoformat(row["created_at"]),
        last_main_sha=row.get("last_main_sha"),
        consecutive_failures=row.get("consecutive_failures", 0),
        pending_activations=row.get("pending_activations", 0),
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
    goal: list[str] = Field(min_length=1)
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
