"""Data structures for lfd daemon."""

from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path

# Re-export Session and SessionStatus from lf.models for backwards compatibility
from loopflow.lf.models import Session as Session
from loopflow.lf.models import SessionStatus as SessionStatus


def area_to_slug(area: str) -> str:
    """Convert area to slug: 'swift/' -> 'swift', '.' -> 'root'."""
    if area == ".":
        return "root"
    return area.rstrip("/").split("/")[-1].lower()


# Run: an execution instance of a Flow


class RunStatus(Enum):
    """Status of a Run execution."""

    PENDING = "pending"  # Created, not yet started
    RUNNING = "running"  # Currently executing
    COMPLETED = "completed"  # Finished successfully
    FAILED = "failed"  # Finished with error
    CANCELLED = "cancelled"  # Stopped before completion


@dataclass
class Run:
    """An execution instance of a Flow.

    Runs can be spawned by a trigger (Loop, Subscription, Schedule)
    or created directly (no parent).
    """

    id: str
    parent: str | None  # "loop:<id>" | "subscription:<id>" | "schedule:<id>" | None

    flow: str  # Flow name (from .lf/flows/)
    area: str  # Area of responsibility
    repo: Path
    goals: list[str] = field(default_factory=list)

    status: RunStatus = RunStatus.PENDING
    iteration: int = 0  # Which iteration of parent (0 for direct runs)

    worktree: str | None = None
    branch: str | None = None
    current_step: str | None = None
    error: str | None = None
    pr_url: str | None = None

    started_at: datetime | None = None
    ended_at: datetime | None = None
    created_at: datetime = field(default_factory=datetime.now)

    def short_id(self) -> str:
        return self.id[:7]

    @property
    def parent_type(self) -> str | None:
        return parse_parent(self.parent)[0]

    @property
    def parent_id(self) -> str | None:
        return parse_parent(self.parent)[1]

    @property
    def goals_display(self) -> str:
        if not self.goals:
            return "adaptive"
        return ", ".join(self.goals)

    @property
    def flow_display(self) -> str:
        return self.flow or "default"


def parse_parent(parent: str | None) -> tuple[str | None, str | None]:
    """Parse parent string into (type, id) tuple."""
    if not parent:
        return None, None
    kind, id = parent.split(":", 1)
    return kind, id


# Trigger status (shared by Loop, Subscription, Schedule)


class TriggerStatus(Enum):
    """Runtime status of a trigger."""

    IDLE = "idle"  # Not running
    RUNNING = "running"  # Currently has an active Run
    WAITING = "waiting"  # Paused (PR limit reached)
    ERROR = "error"  # Last Run failed


class MergeMode(Enum):
    """How iteration branches merge to main."""

    PR = "pr"  # Accumulate on trigger-main, human reviews and lands
    LAND = "land"  # Auto-land to main after each iteration


# Loop: continuously spawns Runs until stopped


@dataclass
class Loop:
    """A continuous runner that spawns Runs."""

    id: str
    flow: str
    area: str
    repo: Path
    goals: list[str] = field(default_factory=list)

    status: TriggerStatus = TriggerStatus.IDLE
    iteration: int = 0

    main_branch: str = ""  # Branch for accumulating work
    pr_limit: int = 5
    merge_mode: MergeMode = MergeMode.PR

    pid: int | None = None
    created_at: datetime = field(default_factory=datetime.now)

    def short_id(self) -> str:
        return self.id[:7]

    @property
    def area_slug(self) -> str:
        return area_to_slug(self.area)

    @property
    def goals_display(self) -> str:
        if not self.goals:
            return "adaptive"
        return ", ".join(self.goals)

    @property
    def flow_display(self) -> str:
        return self.flow or "default"


# Subscription: spawns Run when pathset changes on main


@dataclass
class Subscription:
    """A pathset watcher that spawns Runs when files change."""

    id: str
    flow: str
    area: str
    repo: Path
    goals: list[str] = field(default_factory=list)

    pathset: str = ""  # Comma-separated paths to watch
    last_main_sha: str | None = None

    status: TriggerStatus = TriggerStatus.IDLE
    iteration: int = 0

    main_branch: str = ""
    pr_limit: int = 5
    merge_mode: MergeMode = MergeMode.PR

    pid: int | None = None
    created_at: datetime = field(default_factory=datetime.now)

    def short_id(self) -> str:
        return self.id[:7]

    @property
    def area_slug(self) -> str:
        return area_to_slug(self.area)

    @property
    def goals_display(self) -> str:
        if not self.goals:
            return "adaptive"
        return ", ".join(self.goals)

    @property
    def flow_display(self) -> str:
        return self.flow or "default"


# Schedule: spawns Run on cron


@dataclass
class Schedule:
    """A cron trigger that spawns Runs on schedule."""

    id: str
    flow: str
    area: str
    repo: Path
    goals: list[str] = field(default_factory=list)

    cron: str = ""  # Cron expression

    status: TriggerStatus = TriggerStatus.IDLE
    iteration: int = 0

    main_branch: str = ""
    pr_limit: int = 5
    merge_mode: MergeMode = MergeMode.PR

    pid: int | None = None
    created_at: datetime = field(default_factory=datetime.now)

    def short_id(self) -> str:
        return self.id[:7]

    @property
    def area_slug(self) -> str:
        return area_to_slug(self.area)

    @property
    def goals_display(self) -> str:
        if not self.goals:
            return "adaptive"
        return ", ".join(self.goals)

    @property
    def flow_display(self) -> str:
        return self.flow or "default"


# Type alias for any trigger
Trigger = Loop | Subscription | Schedule
