"""Data structures for lfd daemon."""

from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Literal


class TriggerKind(Enum):
    MANUAL = "manual"
    MAIN_CHANGED = "main-changed"
    INTERVAL = "interval"


class AgentStatus(Enum):
    IDLE = "idle"
    RUNNING = "running"
    WAITING = "waiting"
    ERROR = "error"
    STOPPED = "stopped"


class SessionStatus(Enum):
    RUNNING = "running"
    WAITING = "waiting"
    COMPLETED = "completed"
    ERROR = "error"


@dataclass
class TriggerSpec:
    kind: TriggerKind = TriggerKind.MANUAL
    interval_seconds: int | None = None

    def to_dict(self) -> dict:
        result = {"kind": self.kind.value}
        if self.interval_seconds is not None:
            result["interval_seconds"] = self.interval_seconds
        return result

    @classmethod
    def from_dict(cls, data: dict) -> "TriggerSpec":
        return cls(
            kind=TriggerKind(data.get("kind", "manual")),
            interval_seconds=data.get("interval_seconds"),
        )


@dataclass
class AgentSpec:
    name: str
    repo: Path
    pipeline: list[str]
    trigger: TriggerSpec = field(default_factory=TriggerSpec)
    context: list[str] = field(default_factory=list)
    prompt: str = ""

    def to_dict(self) -> dict:
        return {
            "name": self.name,
            "repo": str(self.repo),
            "pipeline": self.pipeline,
            "trigger": self.trigger.to_dict(),
            "context": self.context,
            "prompt": self.prompt,
        }

    @classmethod
    def from_dict(cls, data: dict) -> "AgentSpec":
        trigger_data = data.get("trigger", {})
        return cls(
            name=data["name"],
            repo=Path(data["repo"]),
            pipeline=data["pipeline"],
            trigger=TriggerSpec.from_dict(trigger_data) if trigger_data else TriggerSpec(),
            context=data.get("context", []),
            prompt=data.get("prompt", ""),
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

    def to_dict(self) -> dict:
        return {
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
        )


@dataclass
class Session:
    id: str
    task: str
    repo: str
    worktree: str
    status: SessionStatus
    started_at: datetime
    ended_at: datetime | None = None
    pid: int | None = None
    model: str = "claude-code"
    run_mode: Literal["auto", "interactive"] = "auto"

    def to_dict(self) -> dict:
        return {
            "id": self.id,
            "task": self.task,
            "repo": self.repo,
            "worktree": self.worktree,
            "status": self.status.value,
            "started_at": self.started_at.isoformat(),
            "ended_at": self.ended_at.isoformat() if self.ended_at else None,
            "pid": self.pid,
            "model": self.model,
            "run_mode": self.run_mode,
        }

    @classmethod
    def from_dict(cls, data: dict) -> "Session":
        return cls(
            id=data["id"],
            task=data["task"],
            repo=data["repo"],
            worktree=data["worktree"],
            status=SessionStatus(data["status"]),
            started_at=datetime.fromisoformat(data["started_at"]),
            ended_at=datetime.fromisoformat(data["ended_at"]) if data.get("ended_at") else None,
            pid=data.get("pid"),
            model=data.get("model", "claude-code"),
            run_mode=data.get("run_mode", "auto"),
        )
