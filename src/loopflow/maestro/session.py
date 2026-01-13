"""Session tracking data structures."""

from dataclasses import dataclass, asdict
from datetime import datetime
from enum import Enum
from pathlib import Path


class SessionStatus(Enum):
    RUNNING = "running"
    WAITING = "waiting"
    COMPLETED = "completed"
    ERROR = "error"


@dataclass
class Session:
    id: str
    task: str
    repo: Path
    worktree: Path
    status: SessionStatus
    started_at: datetime
    ended_at: datetime | None = None
    pid: int | None = None
    backend: str = "claude-code"
    run_mode: str = "auto"

    def to_dict(self) -> dict:
        """Serialize to dict for JSON storage."""
        d = asdict(self)
        d["repo"] = str(self.repo)
        d["worktree"] = str(self.worktree)
        d["status"] = self.status.value
        d["started_at"] = self.started_at.isoformat()
        if self.ended_at:
            d["ended_at"] = self.ended_at.isoformat()
        d["run_mode"] = self.run_mode
        return d

    @classmethod
    def from_dict(cls, data: dict) -> "Session":
        """Deserialize from dict."""
        data = data.copy()
        data["repo"] = Path(data["repo"])
        data["worktree"] = Path(data["worktree"])
        data["status"] = SessionStatus(data["status"])
        data["started_at"] = datetime.fromisoformat(data["started_at"])
        if data.get("ended_at"):
            data["ended_at"] = datetime.fromisoformat(data["ended_at"])
        # Handle db column name mismatch
        if "model" in data:
            data["backend"] = data.pop("model")
        if "run_mode" not in data:
            data["run_mode"] = "auto"
        return cls(**data)
