"""Data structures for lfd daemon."""

from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path

# Re-export Session and SessionStatus from lf.models for backwards compatibility
from loopflow.lf.models import Session, SessionStatus


# New loop-based models


class LoopType(Enum):
    """Type of loop execution."""
    LOOP = "loop"          # Continuous homeostasis
    FLOW = "flow"          # One-off project execution
    SUBSCRIBE = "subscribe"  # Triggered by pathset changes on main
    SCHEDULE = "schedule"    # Triggered by cron


class LoopStatus(Enum):
    """Runtime status of a loop."""
    IDLE = "idle"          # Not running
    RUNNING = "running"    # Currently executing an iteration
    WAITING = "waiting"    # Paused (outstanding >= limit)
    ERROR = "error"        # Last iteration failed


class MergeMode(Enum):
    """How iteration branches merge to personal-main."""
    AUTO = "auto"    # PR auto-merges to personal-main (default)
    PR = "pr"        # PR to personal-main waits for approval
    LAND = "land"    # Auto-merge to personal-main AND to main


@dataclass
class Loop:
    """A loop configuration (goal + repo combination)."""
    id: str
    type: LoopType
    goal: str
    repo: Path
    personal_main: str
    status: LoopStatus = LoopStatus.IDLE
    iteration: int = 0
    pr_limit: int = 5
    merge_mode: MergeMode = MergeMode.AUTO

    # Type-specific config
    project_file: str | None = None   # for flow
    pathset: str | None = None        # for subscribe (comma-separated)
    cron: str | None = None           # for schedule
    area: str | None = None           # area of responsibility override

    pid: int | None = None            # process ID when running
    created_at: datetime = field(default_factory=datetime.now)

    def short_id(self) -> str:
        """Return first 7 chars of ID (like git)."""
        return self.id[:7]


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


# Legacy models (kept for backwards compatibility during migration)


class TriggerKind(Enum):
    MANUAL = "manual"
    MAIN_CHANGED = "main-changed"
    INTERVAL = "interval"
    LOOP = "loop"
    CRON = "cron"


class AgentStatus(Enum):
    IDLE = "idle"
    RUNNING = "running"
    WAITING = "waiting"
    ERROR = "error"
    STOPPED = "stopped"


@dataclass
class TriggerSpec:
    kind: TriggerKind = TriggerKind.MANUAL
    interval_seconds: int | None = None
    cron: str | None = None
    grace_minutes: int = 60

    def to_dict(self) -> dict:
        result = {"kind": self.kind.value}
        if self.interval_seconds is not None:
            result["interval_seconds"] = self.interval_seconds
        if self.cron is not None:
            result["cron"] = self.cron
        if self.grace_minutes != 60:
            result["grace_minutes"] = self.grace_minutes
        return result

    @classmethod
    def from_dict(cls, data: dict) -> "TriggerSpec":
        return cls(
            kind=TriggerKind(data.get("kind", "manual")),
            interval_seconds=data.get("interval_seconds"),
            cron=data.get("cron"),
            grace_minutes=data.get("grace_minutes", 60),
        )


# Note: MergeMode is defined above for loops. Legacy code may use SILENT which maps to AUTO.


@dataclass
class AgentSpec:
    name: str
    repo: Path
    pipeline: str  # Pipeline name to run
    trigger: TriggerSpec = field(default_factory=TriggerSpec)
    context: list[str] = field(default_factory=list)
    prompt: str = ""
    emoji: str = ""
    goal: str = ""  # Name of goal file in .lf/goals/{goal}.md
    area: list[str] = field(default_factory=list)  # Pathset - where agent works
    merge_mode: MergeMode = MergeMode.AUTO
    personal_main: str | None = None  # e.g. "myagent-main" or "myagent-1-main"

    def to_dict(self) -> dict:
        result = {
            "name": self.name,
            "repo": str(self.repo),
            "pipeline": self.pipeline,
            "trigger": self.trigger.to_dict(),
            "context": self.context,
            "prompt": self.prompt,
        }
        if self.emoji:
            result["emoji"] = self.emoji
        if self.goal:
            result["goal"] = self.goal
        if self.area:
            result["area"] = self.area
        if self.merge_mode != MergeMode.AUTO:
            result["merge_mode"] = self.merge_mode.value
        if self.personal_main:
            result["personal_main"] = self.personal_main
        return result

    @classmethod
    def from_dict(cls, data: dict) -> "AgentSpec":
        trigger_data = data.get("trigger", {})
        merge = data.get("merge_mode", "auto")
        return cls(
            name=data["name"],
            repo=Path(data["repo"]),
            pipeline=data["pipeline"],
            trigger=TriggerSpec.from_dict(trigger_data) if trigger_data else TriggerSpec(),
            context=data.get("context", []),
            prompt=data.get("prompt", ""),
            emoji=data.get("emoji", ""),
            goal=data.get("goal", ""),
            area=data.get("area", []),
            merge_mode=MergeMode(merge),
            personal_main=data.get("personal_main"),
        )


@dataclass
class AgentRun:
    id: str
    agent_name: str
    status: AgentStatus
    started_at: datetime
    ended_at: datetime | None = None
    pid: int | None = None
    worktree: str | None = None
    iteration: int = 0
    error: str | None = None
    main_sha: str | None = None
    emoji: str = ""
    current_step: str | None = None  # Which task is currently running

    def to_dict(self) -> dict:
        result = {
            "id": self.id,
            "agent_name": self.agent_name,
            "status": self.status.value,
            "started_at": self.started_at.isoformat(),
            "ended_at": self.ended_at.isoformat() if self.ended_at else None,
            "pid": self.pid,
            "worktree": self.worktree,
            "iteration": self.iteration,
            "error": self.error,
            "main_sha": self.main_sha,
        }
        if self.emoji:
            result["emoji"] = self.emoji
        if self.current_step:
            result["current_step"] = self.current_step
        return result

    @classmethod
    def from_dict(cls, data: dict) -> "AgentRun":
        return cls(
            id=data["id"],
            agent_name=data["agent_name"],
            status=AgentStatus(data["status"]),
            started_at=datetime.fromisoformat(data["started_at"]),
            ended_at=datetime.fromisoformat(data["ended_at"]) if data.get("ended_at") else None,
            pid=data.get("pid"),
            worktree=data.get("worktree"),
            iteration=data.get("iteration", 0),
            error=data.get("error"),
            main_sha=data.get("main_sha"),
            emoji=data.get("emoji", ""),
            current_step=data.get("current_step"),
        )


