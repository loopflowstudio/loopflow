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


class StimulusKind(str, Enum):
    """Type of stimulus trigger."""

    ONCE = "once"  # Single run (one-shot)
    LOOP = "loop"  # Continuously until stopped
    WATCH = "watch"  # When files in area change on main
    CRON = "cron"  # On schedule


@dataclass
class Stimulus:
    """An independent trigger that can activate a wave.

    Multiple stimuli can point to the same wave (many:1).
    Wave owns the "what" (area, direction, flow).
    Stimulus owns the "when" (kind, trigger config, state).
    """

    id: str
    wave_id: str
    kind: Literal["once", "loop", "watch", "cron"]

    # Config (kind-specific)
    cron: str | None = None  # Required when kind = cron

    # State (kind-specific, tracked per-stimulus)
    last_main_sha: str | None = None  # For watch: last seen SHA on main
    last_triggered_at: datetime | None = None  # For cron: last trigger time

    # Metadata
    enabled: bool = True
    created_at: datetime | None = None

    def __str__(self) -> str:
        if self.kind == "cron" and self.cron:
            return f"cron({self.cron})"
        return self.kind

    def short_id(self) -> str:
        return self.id[:7]


@dataclass
class PendingActivation:
    """Tracks queued trigger events for coalescing.

    When multiple triggers fire while a wave is running, they queue here
    and coalesce into a single activation with combined context.
    """

    id: str
    wave_id: str
    stimulus_id: str
    from_sha: str = ""  # For watch: start of diff range
    to_sha: str = ""  # For watch: end of diff range
    queued_at: datetime | None = None


class WaveStatus(str, Enum):
    """Runtime status of a wave."""

    IDLE = "idle"
    RUNNING = "running"
    WAITING = "waiting"
    ERROR = "error"


class MergeMode(str, Enum):
    """How iteration branches merge to main."""

    PR = "pr"
    LAND = "land"


class Wave(LfdModel):
    """An orchestrated unit of autonomous work.

    Wave owns the "what" (area, direction, flow).
    Stimuli (separate entities) own the "when" (trigger config, state).
    Multiple stimuli can point to the same wave (many:1).

    area and direction are optional at creation time and validated at run-time.
    """

    id: str
    name: str  # unique name, used for worktree/branch naming
    repo: Path
    flow: str = "design"  # flow or step name
    direction: list[str] | None = None  # optional, validated at run-time
    area: list[str] | None = None  # optional, validated at run-time

    # Note: stimulus field removed - now separate Stimulus entities
    # Multiple Stimulus objects can reference this wave via wave_id

    paused: bool = True  # when paused, stimuli don't fire (manual mode)
    status: WaveStatus = WaveStatus.IDLE
    iteration: int = 0

    worktree: Path | None = None  # persistent worktree location
    branch: str | None = None  # current branch name
    pr_limit: int = Field(default=5, ge=1)
    merge_mode: MergeMode = MergeMode.PR

    pid: int | None = None
    created_at: datetime = Field(default_factory=datetime.now)

    # Note: last_main_sha moved to Stimulus (per-stimulus state)

    # Circuit breaker
    consecutive_failures: int = 0

    # Activation queue (count of pending_activations table entries)
    pending_activations: int = 0

    # Stacking support
    base_branch: str | None = None  # branch this wave is stacked on
    base_commit: str | None = None  # SHA when branched (for squash merge recovery)

    # Step execution state
    step_index: int = 0

    model_config = ConfigDict(
        extra="forbid",
        validate_assignment=True,
        arbitrary_types_allowed=True,
    )

    @field_validator("direction", mode="before")
    @classmethod
    def normalize_direction(cls, v):
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

    def short_id(self) -> str:
        return self.id[:7]

    @property
    def main_branch(self) -> str:
        """Main branch is {name}.main."""
        return f"{self.name}.main"

    @property
    def direction_display(self) -> str:
        if self.direction is None:
            return ""
        return ", ".join(self.direction)

    @property
    def area_display(self) -> str:
        if self.area is None:
            return ""
        return ", ".join(self.area)

    @property
    def area_slug(self) -> str:
        """Short slug from area for PR titles: 'swift/' -> 'swift', '.' -> 'root'."""
        if not self.area:
            return "root"
        return area_to_slug(self.area[0])

    def is_configured(self) -> bool:
        """Check if wave has required config for running."""
        return self.area is not None


def wave_from_row(row: dict) -> Wave:
    """Convert database row to Wave."""
    direction_str = row.get("direction")
    direction = json.loads(direction_str) if direction_str else None

    area_str = row.get("area")
    area = json.loads(area_str) if area_str else None

    merge_mode_str = row.get("merge_mode", "pr")
    if merge_mode_str == "auto":
        merge_mode_str = "pr"

    # Note: stimulus and last_main_sha now in separate stimuli table

    worktree_str = row.get("worktree")
    worktree = Path(worktree_str) if worktree_str else None

    # Default paused=True for new waves (manual mode)
    paused = row.get("paused")
    if paused is None:
        paused = True
    else:
        paused = bool(paused)

    return Wave(
        id=row["id"],
        name=row["name"],
        repo=Path(row["repo"]),
        flow=row["flow"],
        direction=direction,
        area=area,
        paused=paused,
        status=WaveStatus(row["status"]),
        iteration=row.get("iteration", 0),
        worktree=worktree,
        branch=row.get("branch"),
        pr_limit=row.get("pr_limit", 5),
        merge_mode=MergeMode(merge_mode_str),
        pid=row.get("pid"),
        created_at=datetime.fromisoformat(row["created_at"]),
        consecutive_failures=row.get("consecutive_failures", 0),
        pending_activations=row.get("pending_activations", 0),
        base_branch=row.get("base_branch"),
        base_commit=row.get("base_commit"),
        step_index=row.get("step_index", 0),
    )


def stimulus_from_row(row: dict) -> Stimulus:
    """Convert database row to Stimulus."""
    last_triggered_at = row.get("last_triggered_at")
    if last_triggered_at is not None:
        last_triggered_at = datetime.fromtimestamp(last_triggered_at)

    created_at = row.get("created_at")
    if created_at is not None:
        created_at = datetime.fromisoformat(created_at)

    return Stimulus(
        id=row["id"],
        wave_id=row["wave_id"],
        kind=row["kind"],
        cron=row.get("cron") or None,
        last_main_sha=row.get("last_main_sha"),
        last_triggered_at=last_triggered_at,
        enabled=bool(row.get("enabled", 1)),
        created_at=created_at,
    )


def pending_activation_from_row(row: dict) -> PendingActivation:
    """Convert database row to PendingActivation."""
    queued_at = row.get("queued_at")
    if queued_at is not None:
        queued_at = datetime.fromtimestamp(queued_at)

    return PendingActivation(
        id=row["id"],
        wave_id=row["wave_id"],
        stimulus_id=row["stimulus_id"],
        from_sha=row.get("from_sha") or "",
        to_sha=row.get("to_sha") or "",
        queued_at=queued_at,
    )


# FlowRun: an execution instance of a Flow


class FlowRunStatus(str, Enum):
    """Status of a FlowRun execution."""

    PENDING = "pending"
    RUNNING = "running"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


class TickResult(str, Enum):
    """Result of a single tick_flow() call."""

    STEP_COMPLETE = "step_complete"  # Continue ticking
    FLOW_COMPLETE = "flow_complete"  # Flow finished successfully
    WAITING_INTERACTIVE = "waiting_interactive"  # Paused at interactive step
    STEP_FAILED = "step_failed"  # Step failed


class FlowRun(LfdModel):
    """An execution instance of a Flow, spawned by a Wave."""

    id: str
    wave_id: str | None = None

    flow: str
    direction: list[str] = Field(min_length=1)
    area: list[str] = Field(min_length=1)
    repo: Path

    status: FlowRunStatus = FlowRunStatus.PENDING
    iteration: int = 0
    step_index: int = 0  # Position in flow.steps list for tick-based execution

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

    Can belong to a FlowRun (wave-spawned) or be standalone (interactive).
    """

    id: str
    step: str
    repo: str
    worktree: str

    flow_run_id: str | None = None
    wave_id: str | None = None

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
            "wave_id": self.wave_id,
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
            wave_id=data.get("wave_id"),
            status=StepRunStatus(data["status"]),
            started_at=datetime.fromisoformat(data["started_at"]),
            ended_at=datetime.fromisoformat(data["ended_at"]) if data.get("ended_at") else None,
            pid=data.get("pid"),
            model=data.get("model", "claude-code"),
            run_mode=data.get("run_mode", "auto"),
        )
