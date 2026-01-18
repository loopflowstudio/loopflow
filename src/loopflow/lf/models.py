"""Data structures for lf task execution."""

from dataclasses import dataclass
from datetime import datetime
from enum import Enum
from typing import Literal


class SessionStatus(Enum):
    RUNNING = "running"
    WAITING = "waiting"
    COMPLETED = "completed"
    ERROR = "error"


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
