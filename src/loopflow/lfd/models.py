"""Data structures for lfd daemon."""

from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path

# Re-export Session and SessionStatus from lf.models for backwards compatibility
from loopflow.lf.models import Session as Session
from loopflow.lf.models import SessionStatus as SessionStatus

# New loop-based models


class LoopType(Enum):
    """Type of loop execution."""

    LOOP = "loop"  # Continuous homeostasis
    FLOW = "flow"  # One-off project execution
    SUBSCRIBE = "subscribe"  # Triggered by pathset changes on main
    SCHEDULE = "schedule"  # Triggered by cron


class LoopStatus(Enum):
    """Runtime status of a loop."""

    IDLE = "idle"  # Not running
    RUNNING = "running"  # Currently executing an iteration
    WAITING = "waiting"  # Paused (outstanding >= limit)
    ERROR = "error"  # Last iteration failed


class MergeMode(Enum):
    """How iteration branches merge to loop-main."""

    PR = "pr"  # Accumulate on loop-main, human reviews and lands
    LAND = "land"  # Auto-land to main after each iteration


@dataclass
class Loop:
    """A loop configuration (area + flow + goals combination)."""

    id: str
    type: LoopType
    area: str  # PRIMARY identifier, required (e.g., "Maestro/", ".", "src/loopflow/")
    repo: Path
    loop_main: str
    flow: str | None = None  # pipeline/flow name (e.g. "ship")
    goals: list[str] = field(default_factory=list)  # goal names from -g flags
    status: LoopStatus = LoopStatus.IDLE
    iteration: int = 0
    pr_limit: int = 5
    merge_mode: MergeMode = MergeMode.PR

    # Type-specific config
    project_file: str | None = None  # for flow
    pathset: str | None = None  # for subscribe (comma-separated)
    cron: str | None = None  # for schedule

    # Legacy field for backwards compat (deprecated, use goals)
    goal_name: str | None = None

    pid: int | None = None  # process ID when running
    last_main_sha: str | None = None  # for subscribe: last seen main SHA
    created_at: datetime = field(default_factory=datetime.now)

    def short_id(self) -> str:
        """Return first 7 chars of ID (like git)."""
        return self.id[:7]

    @property
    def area_slug(self) -> str:
        """Return area as a slug for display/naming."""
        if self.area == ".":
            return "root"
        return self.area.rstrip("/").split("/")[-1]

    @property
    def goals_display(self) -> str:
        """Return goals as comma-separated string for display."""
        if not self.goals:
            return "adaptive"
        return ", ".join(self.goals)

    @property
    def flow_display(self) -> str:
        """Return flow display string."""
        return self.flow or "default"


@dataclass
class LoopRun:
    """A single iteration attempt."""

    id: str
    loop_id: str
    iteration: int
    status: LoopStatus
    started_at: datetime
    ended_at: datetime | None = None
    worktree: str | None = None
    current_step: str | None = None
    error: str | None = None
    pr_url: str | None = None
