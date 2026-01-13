"""Agent loop data structures and core logic.

Background agents run a configurable inner pipeline with a persistent prompt.
They are registered through Maestro and stored in SQLite.
"""

from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Optional
import json


class OuterLoopMode(Enum):
    PR_CHAIN = "pr-chain"
    LAND_COMMITS = "land-commits"


class AgentStatus(Enum):
    IDLE = "idle"
    RUNNING = "running"
    ERROR = "error"


@dataclass
class OuterLoopConfig:
    """Configuration for the outer loop behavior."""

    mode: OuterLoopMode

    def to_dict(self) -> dict:
        return {"mode": self.mode.value}

    @classmethod
    def from_dict(cls, data: dict) -> "OuterLoopConfig":
        return cls(mode=OuterLoopMode(data["mode"]))


@dataclass
class AgentLoopSpec:
    """Specification for a background agent loop."""

    name: str
    prompt_path: Path
    pipeline: list[str]
    context: list[str] = field(default_factory=list)
    outer_loop: OuterLoopConfig = field(
        default_factory=lambda: OuterLoopConfig(mode=OuterLoopMode.LAND_COMMITS)
    )

    def to_dict(self) -> dict:
        return {
            "name": self.name,
            "prompt_path": str(self.prompt_path),
            "pipeline": self.pipeline,
            "context": self.context,
            "outer_loop": self.outer_loop.to_dict(),
        }

    @classmethod
    def from_dict(cls, data: dict) -> "AgentLoopSpec":
        return cls(
            name=data["name"],
            prompt_path=Path(data["prompt_path"]),
            pipeline=data["pipeline"],
            context=data.get("context", []),
            outer_loop=OuterLoopConfig.from_dict(data["outer_loop"]),
        )


@dataclass
class RegisteredAgent:
    """A registered background agent with runtime state."""

    id: str
    spec: AgentLoopSpec
    status: AgentStatus = AgentStatus.IDLE
    last_run_at: Optional[datetime] = None
    current_worktree: Optional[Path] = None
    current_branch: Optional[str] = None
    iteration: int = 0
    pid: Optional[int] = None

    def to_dict(self) -> dict:
        return {
            "id": self.id,
            "spec": self.spec.to_dict(),
            "status": self.status.value,
            "last_run_at": self.last_run_at.isoformat() if self.last_run_at else None,
            "current_worktree": str(self.current_worktree) if self.current_worktree else None,
            "current_branch": self.current_branch,
            "iteration": self.iteration,
            "pid": self.pid,
        }

    @classmethod
    def from_dict(cls, data: dict) -> "RegisteredAgent":
        return cls(
            id=data["id"],
            spec=AgentLoopSpec.from_dict(data["spec"]),
            status=AgentStatus(data["status"]),
            last_run_at=datetime.fromisoformat(data["last_run_at"]) if data.get("last_run_at") else None,
            current_worktree=Path(data["current_worktree"]) if data.get("current_worktree") else None,
            current_branch=data.get("current_branch"),
            iteration=data.get("iteration", 0),
            pid=data.get("pid"),
        )
