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


class ExecutionMode(Enum):
    """How many times the agent runs."""

    CONTINUOUS = "continuous"  # Keep iterating until stopped
    ONE_SHOT = "one_shot"  # Run once, then done


class TriggerType(Enum):
    """What causes the agent to run."""

    MANUAL = "manual"  # Started explicitly
    PATHSET = "pathset"  # File changes on main
    CRON = "cron"  # Scheduled time


class JobType(Enum):
    """Legacy type enum - combines execution mode and trigger.

    Deprecated: Use execution_mode and trigger_type instead.
    Kept for backwards compatibility with database and API.
    """

    LOOP = "loop"  # CONTINUOUS + MANUAL
    FLOW = "flow"  # ONE_SHOT + MANUAL
    SUBSCRIBE = "subscribe"  # CONTINUOUS + PATHSET
    SCHEDULE = "schedule"  # CONTINUOUS + CRON


# Backwards compatibility alias
LoopType = JobType


def _job_type_to_mode_trigger(job_type: JobType) -> tuple[ExecutionMode, TriggerType]:
    """Convert legacy JobType to (ExecutionMode, TriggerType)."""
    mapping = {
        JobType.LOOP: (ExecutionMode.CONTINUOUS, TriggerType.MANUAL),
        JobType.FLOW: (ExecutionMode.ONE_SHOT, TriggerType.MANUAL),
        JobType.SUBSCRIBE: (ExecutionMode.CONTINUOUS, TriggerType.PATHSET),
        JobType.SCHEDULE: (ExecutionMode.CONTINUOUS, TriggerType.CRON),
    }
    return mapping[job_type]


def _mode_trigger_to_job_type(mode: ExecutionMode, trigger: TriggerType) -> JobType:
    """Convert (ExecutionMode, TriggerType) to legacy JobType."""
    mapping = {
        (ExecutionMode.CONTINUOUS, TriggerType.MANUAL): JobType.LOOP,
        (ExecutionMode.ONE_SHOT, TriggerType.MANUAL): JobType.FLOW,
        (ExecutionMode.CONTINUOUS, TriggerType.PATHSET): JobType.SUBSCRIBE,
        (ExecutionMode.CONTINUOUS, TriggerType.CRON): JobType.SCHEDULE,
        # New combinations default to ONE_SHOT variants
        (ExecutionMode.ONE_SHOT, TriggerType.PATHSET): JobType.SUBSCRIBE,
        (ExecutionMode.ONE_SHOT, TriggerType.CRON): JobType.SCHEDULE,
    }
    return mapping[(mode, trigger)]


class JobStatus(Enum):
    """Runtime status of a job."""

    IDLE = "idle"  # Not running
    RUNNING = "running"  # Currently executing an iteration
    WAITING = "waiting"  # Paused (outstanding >= limit)
    ERROR = "error"  # Last iteration failed


# Backwards compatibility alias
LoopStatus = JobStatus


class MergeMode(Enum):
    """How iteration branches merge to job-main."""

    PR = "pr"  # Accumulate on job-main, human reviews and lands
    LAND = "land"  # Auto-land to main after each iteration


@dataclass
class Job:
    """A job configuration (area + flow + goals combination)."""

    id: str
    type: JobType
    area: str  # PRIMARY identifier, required (e.g., "swift/", ".", "src/loopflow/")
    repo: Path
    job_main: str
    flow: str | None = None  # pipeline/flow name (e.g. "ship")
    goals: list[str] = field(default_factory=list)  # goal names from -g flags
    status: JobStatus = JobStatus.IDLE
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

    def __init__(
        self,
        id: str,
        type: JobType,
        area: str,
        repo: Path,
        job_main: str | None = None,
        loop_main: str | None = None,  # backwards compat alias
        flow: str | None = None,
        goals: list[str] | None = None,
        status: JobStatus = JobStatus.IDLE,
        iteration: int = 0,
        pr_limit: int = 5,
        merge_mode: MergeMode = MergeMode.PR,
        project_file: str | None = None,
        pathset: str | None = None,
        cron: str | None = None,
        goal_name: str | None = None,
        pid: int | None = None,
        last_main_sha: str | None = None,
        created_at: datetime | None = None,
    ):
        self.id = id
        self.type = type
        self.area = area
        self.repo = repo
        # Accept either job_main or loop_main (backwards compat)
        self.job_main = job_main if job_main is not None else (loop_main or "")
        self.flow = flow
        self.goals = goals if goals is not None else []
        self.status = status
        self.iteration = iteration
        self.pr_limit = pr_limit
        self.merge_mode = merge_mode
        self.project_file = project_file
        self.pathset = pathset
        self.cron = cron
        self.goal_name = goal_name
        self.pid = pid
        self.last_main_sha = last_main_sha
        self.created_at = created_at if created_at is not None else datetime.now()

    def short_id(self) -> str:
        """Return first 7 chars of ID (like git)."""
        return self.id[:7]

    @property
    def area_slug(self) -> str:
        """Return area as a lowercase slug for display/naming."""
        return area_to_slug(self.area)

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

    @property
    def execution_mode(self) -> ExecutionMode:
        """Whether this runs once or continuously."""
        mode, _ = _job_type_to_mode_trigger(self.type)
        return mode

    @property
    def trigger_type(self) -> TriggerType:
        """What causes this to run."""
        _, trigger = _job_type_to_mode_trigger(self.type)
        return trigger

    @property
    def is_one_shot(self) -> bool:
        """True if this runs once and stops (FLOW type)."""
        return self.execution_mode == ExecutionMode.ONE_SHOT

    @property
    def is_triggered(self) -> bool:
        """True if this waits for external trigger (SUBSCRIBE, SCHEDULE)."""
        return self.trigger_type != TriggerType.MANUAL

    # Backwards compatibility: allow access via loop_main
    @property
    def loop_main(self) -> str:
        return self.job_main

    @loop_main.setter
    def loop_main(self, value: str) -> None:
        self.job_main = value


# Backwards compatibility alias
Loop = Job


@dataclass
class JobRun:
    """A single iteration attempt."""

    id: str
    job_id: str
    iteration: int
    status: JobStatus
    started_at: datetime
    ended_at: datetime | None = None
    worktree: str | None = None
    current_step: str | None = None
    error: str | None = None
    pr_url: str | None = None

    def __init__(
        self,
        id: str,
        job_id: str | None = None,
        loop_id: str | None = None,  # backwards compat alias
        iteration: int = 0,
        status: JobStatus = JobStatus.IDLE,
        started_at: datetime | None = None,
        ended_at: datetime | None = None,
        worktree: str | None = None,
        current_step: str | None = None,
        error: str | None = None,
        pr_url: str | None = None,
    ):
        self.id = id
        # Accept either job_id or loop_id (backwards compat)
        self.job_id = job_id if job_id is not None else (loop_id or "")
        self.iteration = iteration
        self.status = status
        self.started_at = started_at if started_at is not None else datetime.now()
        self.ended_at = ended_at
        self.worktree = worktree
        self.current_step = current_step
        self.error = error
        self.pr_url = pr_url

    # Backwards compatibility
    @property
    def loop_id(self) -> str:
        return self.job_id

    @loop_id.setter
    def loop_id(self, value: str) -> None:
        self.job_id = value


# Backwards compatibility alias
LoopRun = JobRun
