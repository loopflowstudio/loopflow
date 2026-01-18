"""Data structures and logging for lf task execution."""

import json
import socket
from dataclasses import dataclass
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Any, Literal

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"


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


# Session logging (fire-and-forget communication with lfd daemon)


def _send_fire_and_forget(method: str, params: dict[str, Any]) -> None:
    """Send a request to lfd without waiting for response. Fails silently."""
    try:
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(0.5)
        sock.connect(str(SOCKET_PATH))
        request = json.dumps({"method": method, "params": params}) + "\n"
        sock.sendall(request.encode())
        sock.close()
    except Exception:
        pass  # Fire-and-forget: don't block on errors


def log_session_start(session: Session) -> None:
    """Tell lfd a session started. Fire-and-forget."""
    _send_fire_and_forget("sessions.start", {"session": session.to_dict()})


def log_session_end(session_id: str, status: SessionStatus) -> None:
    """Tell lfd a session ended. Fire-and-forget."""
    _send_fire_and_forget("sessions.end", {"session_id": session_id, "status": status.value})
