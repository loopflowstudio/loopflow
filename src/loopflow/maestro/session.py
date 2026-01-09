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
    pid: int | None = None
    backend: str = "claude-code"

    def to_dict(self) -> dict:
        """Serialize to dict for JSON storage."""
        d = asdict(self)
        d["repo"] = str(self.repo)
        d["worktree"] = str(self.worktree)
        d["status"] = self.status.value
        d["started_at"] = self.started_at.isoformat()
        return d

    @classmethod
    def from_dict(cls, data: dict) -> "Session":
        """Deserialize from dict."""
        data = data.copy()
        data["repo"] = Path(data["repo"])
        data["worktree"] = Path(data["worktree"])
        data["status"] = SessionStatus(data["status"])
        data["started_at"] = datetime.fromisoformat(data["started_at"])
        return cls(**data)
